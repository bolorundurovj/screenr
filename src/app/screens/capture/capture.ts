import {Component, computed, inject, OnInit, signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Router} from '@angular/router';
import {TauriService} from '../../core/tauri.service';
import {RecordingService} from '../../core/recording.service';
import {SettingsService} from '../../core/settings.service';
import {ToastService} from '../../core/toast.service';
import {Icon} from '../../shared/icon/icon';
import {ToggleSwitch} from '../../shared/toggle-switch/toggle-switch';
import {Settings, VideoSource} from '../../core/models';

type PickerKind = 'display' | 'window';

@Component({
    selector: 'app-capture',
    imports: [FormsModule, Icon, ToggleSwitch],
    templateUrl: './capture.html',
    host: {
        // Esc is handled here rather than as a global shortcut: claiming it
        // system-wide would swallow it in whatever app is being recorded.
        '(document:keydown.escape)': 'onEscape()',
    },
})
export class Capture implements OnInit {
    private tauri = inject(TauriService);
    private settingsService = inject(SettingsService);
    private toast = inject(ToastService);
    readonly router = inject(Router);
    readonly recording = inject(RecordingService);

    readonly displays = signal<VideoSource[]>([]);
    readonly windows = signal<VideoSource[]>([]);
    readonly windowsLoading = signal(true);

    readonly picker = signal<PickerKind | null>(null);
    readonly selected = signal<ReadonlySet<string>>(new Set());

    readonly settings = this.settingsService.settings;

    readonly pickerSources = computed(() => (this.picker() === 'window' ? this.windows() : this.displays()));

    readonly selectedCount = computed(() => this.selected().size);

    readonly confirmLabel = computed(() => {
        const count = this.selectedCount();
        if (this.picker() === 'window') {
            return count > 1 ? `Record ${count} windows` : 'Record window';
        }
        return count > 1 ? `Record ${count} displays` : 'Record display';
    });

    readonly silent = computed(() => {
        const s = this.settings();
        return !!s && !s.mic;
    });

    async ngOnInit(): Promise<void> {
        // Displays are only a handful of captures and return almost instantly.
        try {
            this.displays.set(await this.tauri.invoke<VideoSource[]>('get_displays'));
        } catch {
            this.toast.show('Could not list displays', 'error');
        }

        // Windows can be dozens of screenshots. Load them in the background so the
        // screen paints immediately rather than freezing on the slow call.
        this.tauri
            .invoke<VideoSource[]>('get_windows')
            .then((windows) => this.windows.set(windows))
            .catch(() => this.toast.show('Could not list windows', 'error'))
            .finally(() => this.windowsLoading.set(false));
    }

    openPicker(kind: PickerKind): void {
        this.selected.set(new Set());
        this.picker.set(kind);
    }

    closePicker(): void {
        this.picker.set(null);
        this.selected.set(new Set());
    }

    onEscape(): void {
        if (this.picker()) {
            this.closePicker();
        } else if (this.recording.active()) {
            void this.recording.stop();
        }
    }

    isSelected(source: VideoSource): boolean {
        return this.selected().has(source.id);
    }

    toggleSelection(source: VideoSource): void {
        this.selected.update((current) => {
            const next = new Set(current);
            if (!next.delete(source.id)) {
                next.add(source.id);
            }
            return next;
        });
    }

    selectAllDisplays(): void {
        this.selected.set(new Set(this.displays().map((display) => display.id)));
    }

    describe(source: VideoSource): string {
        const size = source.width && source.height ? `${source.width}×${source.height}` : '';
        const note = source.isPrimary ? 'Primary' : (source.app ?? '');
        return [size, note].filter(Boolean).join(' · ');
    }

    confirm(): void {
        const chosen = this.pickerSources().filter((source) => this.isSelected(source));
        if (chosen.length === 0) {
            return;
        }
        this.picker.set(null);
        void this.recording.start(chosen);
    }

    async updateAudio(key: 'mic' | 'systemAudio', value: boolean): Promise<void> {
        const current = this.settings();
        if (!current) {
            return;
        }
        const next: Settings = {...current, [key]: value};
        try {
            await this.settingsService.save(next);
        } catch {
            this.toast.show('Could not save audio preference', 'error');
        }
    }
}
