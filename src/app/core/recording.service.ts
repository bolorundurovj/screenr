import {computed, inject, Injectable, signal} from '@angular/core';
import {Router} from '@angular/router';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {TauriService} from './tauri.service';
import {ToastService} from './toast.service';
import {IDLE_RECORDING_STATE, RecordingFinished, RecordingState, VideoSource} from './models';

@Injectable({providedIn: 'root'})
export class RecordingService {
    private tauri = inject(TauriService);
    private router = inject(Router);
    private toast = inject(ToastService);

    readonly state = signal<RecordingState>(IDLE_RECORDING_STATE);
    /** 3, 2, 1 before capture starts; null when no countdown is running. */
    readonly countdown = signal<number | null>(null);

    /**
     * False until ffmpeg is on disk. The first launch downloads it, and
     * recording cannot start before that finishes.
     */
    readonly encoderReady = signal(false);

    /**
     * True from the moment a take is requested until it finishes.
     *
     * `state().isRecording` only turns true once the capture loop emits, which
     * is after the countdown, so it cannot guard against a second start.
     */
    private readonly starting = signal(false);
    readonly active = computed(() => this.starting() || this.state().isRecording);

    /**
     * The overlay runs the same bundle, so this service exists in both windows
     * and every event is delivered to both. Only the main window drives
     * navigation and the shortcut.
     */
    private readonly isMainWindow = getCurrentWindow().label === 'main';

    constructor() {
        this.tauri.listen<RecordingState>('recording_state', ({payload}) => {
            this.state.set(payload);
            // Covers a take cancelled during the countdown, which never reaches
            // recording_finished.
            if (!payload.isRecording) {
                this.starting.set(false);
            }
        });

        this.tauri.listen<number>('countdown_tick', ({payload}) =>
            this.countdown.set(payload === 0 ? null : payload),
        );

        this.tauri.listen<RecordingFinished>('recording_finished', ({payload}) => this.onFinished(payload));

        this.tauri.listen<boolean>('ffmpeg_ready', ({payload}) => this.encoderReady.set(payload));

        // Non-fatal problems such as a missing microphone: the take still runs.
        this.tauri.listen<string>('recording_warning', ({payload}) => {
            if (this.isMainWindow) {
                this.toast.show(payload, 'error');
            }
        });

        if (this.isMainWindow) {
            this.tauri.listen('shortcut_toggle_recording', () => this.toggle());
        }
    }

    async start(sources: VideoSource[]): Promise<void> {
        if (sources.length === 0 || this.active()) {
            return;
        }
        this.starting.set(true);
        try {
            // The backend derives the destination from the configured save folder
            // and format, so the frontend never invents a path.
            await this.tauri.invoke<string>('start_recording', {
                sourceIds: sources.map((source) => source.id),
            });
            await this.tauri.invoke('open_overlay');
        } catch (e) {
            this.reset();
            this.toast.show(describeError(e, 'Could not start recording'), 'error');
        }
    }

    /**
     * Ctrl+Shift+R. Stops a take in progress, otherwise starts one on the
     * primary display so the shortcut works without visiting the picker.
     */
    async toggle(): Promise<void> {
        if (this.active()) {
            await this.stop();
            return;
        }

        try {
            const displays = await this.tauri.invoke<VideoSource[]>('get_displays');
            const primary = displays.find((display) => display.isPrimary) ?? displays[0];
            if (!primary) {
                this.toast.show('No display available to record', 'error');
                return;
            }
            await this.start([primary]);
        } catch (e) {
            this.toast.show(describeError(e, 'Could not start recording'), 'error');
        }
    }

    async pause(): Promise<void> {
        await this.tauri.invoke('pause_recording').catch(() => undefined);
    }

    async resume(): Promise<void> {
        await this.tauri.invoke('resume_recording').catch(() => undefined);
    }

    async stop(): Promise<void> {
        await this.tauri.invoke('stop_recording').catch(() => undefined);
    }

    private async onFinished(finished: RecordingFinished): Promise<void> {
        this.reset();
        if (!this.isMainWindow) {
            return;
        }

        await this.tauri.invoke('close_overlay').catch(() => undefined);
        await this.router.navigate(['/review'], {
            queryParams: {path: finished.path},
        });
    }

    private reset(): void {
        this.starting.set(false);
        this.state.set(IDLE_RECORDING_STATE);
        this.countdown.set(null);
    }
}

/** Tauri command rejections arrive as plain strings. */
export function describeError(error: unknown, fallback: string): string {
    return typeof error === 'string' && error.length > 0 ? error : fallback;
}
