import {Component, computed, inject, OnInit, signal} from '@angular/core';
import {Router} from '@angular/router';
import {TauriService} from '../../core/tauri.service';
import {ToastService} from '../../core/toast.service';
import {describeError} from '../../core/recording.service';
import {WindowFrame} from '../../shared/window-frame/window-frame';
import {Icon} from '../../shared/icon/icon';
import {Take} from '../../core/models';

@Component({
    selector: 'app-library',
    imports: [WindowFrame, Icon],
    templateUrl: './library.html',
})
export class Library implements OnInit {
    private tauri = inject(TauriService);
    private toast = inject(ToastService);
    readonly router = inject(Router);

    readonly takes = signal<Take[]>([]);
    readonly loading = signal(true);
    readonly pendingDelete = signal<string | null>(null);

    readonly summary = computed(() => {
        const takes = this.takes();
        if (takes.length === 0) {
            return 'No recordings yet';
        }
        const total = takes.reduce((sum, take) => sum + take.size, 0);
        const label = takes.length === 1 ? 'take' : 'takes';
        return `${takes.length} ${label} · ${this.formatSize(total)}`;
    });

    async ngOnInit(): Promise<void> {
        await this.load();
    }

    async load(): Promise<void> {
        this.loading.set(true);
        try {
            this.takes.set(await this.tauri.invoke<Take[]>('get_takes'));
        } catch (e) {
            this.toast.show(describeError(e, 'Could not read the library'), 'error');
        } finally {
            this.loading.set(false);
        }
    }

    formatDate(epochSeconds: number): string {
        return new Date(epochSeconds * 1000).toLocaleString(undefined, {
            dateStyle: 'medium',
            timeStyle: 'short',
        });
    }

    formatSize(bytes: number): string {
        if (bytes >= 1024 ** 3) {
            return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
        }
        if (bytes >= 1024 ** 2) {
            return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
        }
        return `${(bytes / 1024).toFixed(0)} KB`;
    }

    open(take: Take): void {
        void this.router.navigate(['/review'], {
            queryParams: {path: take.absolutePath},
        });
    }

    async reveal(take: Take): Promise<void> {
        try {
            await this.tauri.invoke('reveal_take', {path: take.absolutePath});
        } catch (e) {
            this.toast.show(describeError(e, 'Could not open the folder'), 'error');
        }
    }

    async confirmDelete(take: Take): Promise<void> {
        // Inline confirmation instead of a blocking window.confirm dialog.
        if (this.pendingDelete() !== take.absolutePath) {
            this.pendingDelete.set(take.absolutePath);
            return;
        }

        this.pendingDelete.set(null);
        try {
            await this.tauri.invoke('delete_take', {path: take.absolutePath});
            this.takes.update((takes) => takes.filter((t) => t.absolutePath !== take.absolutePath));
            this.toast.show(`Deleted ${take.name}`);
        } catch (e) {
            this.toast.show(describeError(e, 'Could not delete take'), 'error');
        }
    }

    cancelDelete(): void {
        this.pendingDelete.set(null);
    }
}
