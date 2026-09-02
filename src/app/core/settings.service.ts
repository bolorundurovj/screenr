import {computed, inject, Injectable, signal} from '@angular/core';
import {TauriService} from './tauri.service';
import {ThemeService} from './theme.service';
import {Settings} from './models';

@Injectable({providedIn: 'root'})
export class SettingsService {
    private tauri = inject(TauriService);
    private theme = inject(ThemeService);

    private readonly current = signal<Settings | null>(null);

    /** Null until the first load resolves. */
    readonly settings = this.current.asReadonly();
    readonly loaded = computed(() => this.current() !== null);

    /** Called once at startup so every screen sees the right theme. */
    async load(): Promise<void> {
        try {
            const settings = await this.tauri.invoke<Settings>('get_settings');
            this.apply(settings);
        } catch (e) {
            console.error('Failed to load settings', e);
        }
    }

    async save(next: Settings): Promise<Settings> {
        const stored = await this.tauri.invoke<Settings>('save_settings', {
            settings: next,
        });
        this.apply(stored);
        return stored;
    }

    private apply(settings: Settings): void {
        this.current.set(settings);
        this.theme.setTheme(settings.theme);
    }
}
