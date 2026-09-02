import {Component, ElementRef, inject, OnInit, signal, viewChild} from '@angular/core';
import {ActivatedRoute, Router} from '@angular/router';
import {convertFileSrc} from '@tauri-apps/api/core';
import {TauriService} from '../../core/tauri.service';
import {ToastService} from '../../core/toast.service';
import {describeError} from '../../core/recording.service';
import {WindowFrame} from '../../shared/window-frame/window-frame';
import {Icon} from '../../shared/icon/icon';

type ExportKind = 'mp4' | 'gif';

const MIN_TRIM_SECONDS = 0.5;

@Component({
    selector: 'app-review',
    imports: [WindowFrame, Icon],
    templateUrl: './review.html',
})
export class Review implements OnInit {
    private tauri = inject(TauriService);
    private route = inject(ActivatedRoute);
    private toast = inject(ToastService);
    readonly router = inject(Router);

    private readonly player = viewChild<ElementRef<HTMLVideoElement>>('player');

    readonly videoPath = signal('');
    readonly videoUrl = signal('');
    readonly duration = signal(0);
    readonly currentTime = signal(0);
    readonly isPlaying = signal(false);

    readonly trimStart = signal(0);
    readonly trimEnd = signal(0);
    readonly exporting = signal<ExportKind | null>(null);
    readonly loadError = signal<string | null>(null);

    ngOnInit(): void {
        const path = this.route.snapshot.queryParamMap.get('path');
        if (!path) {
            void this.router.navigate(['/library']);
            return;
        }
        this.videoPath.set(path);
        this.videoUrl.set(convertFileSrc(path));
    }

    get fileName(): string {
        return this.videoPath().split(/[\\/]/).pop() ?? '';
    }

    onLoadedMetadata(): void {
        const video = this.player()?.nativeElement;
        if (!video || !Number.isFinite(video.duration)) {
            return;
        }
        this.loadError.set(null);
        this.duration.set(video.duration);
        this.trimStart.set(0);
        this.trimEnd.set(video.duration);
    }

    onLoadError(): void {
        // Most often an unplayable container (MKV) or a file the asset protocol
        // is not scoped to, both of which otherwise fail silently.
        const extension = this.fileName.split('.').pop()?.toLowerCase() ?? '';
        this.loadError.set(
            extension === 'mkv' || extension === 'webm'
                ? `This window cannot play .${extension} files. The take is still saved, and export works.`
                : 'Could not load this take for playback.',
        );
    }

    onTimeUpdate(): void {
        const video = this.player()?.nativeElement;
        if (!video) {
            return;
        }
        this.currentTime.set(video.currentTime);

        // Keep playback inside the selected span so the preview matches the export.
        if (video.currentTime >= this.trimEnd()) {
            video.pause();
            video.currentTime = this.trimStart();
            this.isPlaying.set(false);
        }
    }

    togglePlay(): void {
        const video = this.player()?.nativeElement;
        if (!video) {
            return;
        }
        if (video.paused) {
            if (video.currentTime < this.trimStart() || video.currentTime >= this.trimEnd()) {
                video.currentTime = this.trimStart();
            }
            void video.play();
            this.isPlaying.set(true);
        } else {
            video.pause();
            this.isPlaying.set(false);
        }
    }

    setTrimStart(value: string): void {
        const next = Math.min(Number(value), this.trimEnd() - MIN_TRIM_SECONDS);
        this.trimStart.set(Math.max(0, next));
        this.seekTo(this.trimStart());
    }

    setTrimEnd(value: string): void {
        const next = Math.max(Number(value), this.trimStart() + MIN_TRIM_SECONDS);
        this.trimEnd.set(Math.min(this.duration(), next));
    }

    format(seconds: number): string {
        if (!Number.isFinite(seconds)) {
            return '0:00';
        }
        const minutes = Math.floor(seconds / 60);
        const rest = Math.floor(seconds % 60);
        return `${minutes}:${String(rest).padStart(2, '0')}`;
    }

    async exportAs(kind: ExportKind): Promise<void> {
        if (this.exporting()) {
            return;
        }
        this.exporting.set(kind);
        this.toast.show(kind === 'gif' ? 'Rendering GIF…' : 'Rendering MP4…');

        const command = kind === 'gif' ? 'export_gif' : 'trim_video';
        try {
            await this.tauri.invoke<string>(command, {
                path: this.videoPath(),
                startSecs: this.trimStart(),
                endSecs: this.trimEnd(),
            });
            this.toast.show(
                kind === 'gif' ? 'GIF saved to your library' : 'MP4 saved to your library',
                'success',
            );
        } catch (e) {
            this.toast.show(describeError(e, 'Export failed'), 'error');
        } finally {
            this.exporting.set(null);
        }
    }

    async discard(): Promise<void> {
        try {
            await this.tauri.invoke('delete_take', {path: this.videoPath()});
            this.toast.show('Take discarded');
            await this.router.navigate(['/capture']);
        } catch (e) {
            this.toast.show(describeError(e, 'Could not discard take'), 'error');
        }
    }

    private seekTo(seconds: number): void {
        const video = this.player()?.nativeElement;
        if (video) {
            video.currentTime = seconds;
        }
    }
}
