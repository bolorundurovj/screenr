import {DestroyRef, inject, Injectable, signal} from '@angular/core';
import {Theme} from './models';

const DARK_QUERY = '(prefers-color-scheme: dark)';

@Injectable({providedIn: 'root'})
export class ThemeService {
    private readonly query = window.matchMedia(DARK_QUERY);
    private readonly systemDark = signal(this.query.matches);

    readonly theme = signal<Theme>('system');
    readonly isDark = signal(false);

    constructor() {
        const onSystemChange = (e: MediaQueryListEvent) => {
            this.systemDark.set(e.matches);
            if (this.theme() === 'system') {
                this.applyDark(e.matches);
            }
        };

        this.query.addEventListener('change', onSystemChange);
        inject(DestroyRef).onDestroy(() => this.query.removeEventListener('change', onSystemChange));

        this.applyDark(this.systemDark());
    }

    setTheme(theme: Theme): void {
        this.theme.set(theme);
        this.applyDark(theme === 'dark' || (theme === 'system' && this.systemDark()));
    }

    private applyDark(dark: boolean): void {
        this.isDark.set(dark);
        document.documentElement.classList.toggle('dark', dark);
        document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
    }
}
