import {TestBed} from '@angular/core/testing';
import {beforeEach, describe, expect, it, vi} from 'vitest';
import {ThemeService} from './theme.service';

/** Listeners registered against the prefers-color-scheme query. */
let listeners: ((e: MediaQueryListEvent) => void)[];

/** jsdom has no matchMedia, so the OS preference is stubbed here. */
function stubMatchMedia(systemPrefersDark: boolean): void {
    listeners = [];
    vi.stubGlobal(
        'matchMedia',
        vi.fn(() => ({
            matches: systemPrefersDark,
            addEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) => listeners.push(fn),
            removeEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) => {
                listeners = listeners.filter((l) => l !== fn);
            },
        })),
    );
}

function emitSystemChange(matches: boolean): void {
    listeners.forEach((fn) => fn({matches} as MediaQueryListEvent));
}

function isDark(): boolean {
    return document.documentElement.classList.contains('dark');
}

describe('ThemeService', () => {
    beforeEach(() => {
        document.documentElement.classList.remove('dark');
    });

    function create(systemPrefersDark = false): ThemeService {
        stubMatchMedia(systemPrefersDark);
        TestBed.resetTestingModule();
        TestBed.configureTestingModule({});
        return TestBed.inject(ThemeService);
    }

    it('follows the OS preference on startup', () => {
        create(true);
        expect(isDark()).toBe(true);
    });

    it('stays light when the OS prefers light', () => {
        create(false);
        expect(isDark()).toBe(false);
    });

    it('forces dark regardless of the OS', () => {
        const theme = create(false);
        theme.setTheme('dark');
        expect(isDark()).toBe(true);
        expect(theme.isDark()).toBe(true);
    });

    it('forces light regardless of the OS', () => {
        const theme = create(true);
        theme.setTheme('light');
        expect(isDark()).toBe(false);
    });

    it('reacts to the OS switching while set to system', () => {
        const theme = create(false);
        theme.setTheme('system');

        emitSystemChange(true);
        expect(isDark()).toBe(true);

        emitSystemChange(false);
        expect(isDark()).toBe(false);
    });

    it('ignores OS changes once a theme is pinned', () => {
        const theme = create(false);
        theme.setTheme('light');

        emitSystemChange(true);
        expect(isDark()).toBe(false);
    });

    it('remembers the OS preference so switching back to system applies it', () => {
        const theme = create(false);
        theme.setTheme('light');
        emitSystemChange(true);

        theme.setTheme('system');

        expect(isDark()).toBe(true);
    });

    it('keeps colorScheme in step so native controls match', () => {
        const theme = create(false);
        theme.setTheme('dark');
        expect(document.documentElement.style.colorScheme).toBe('dark');

        theme.setTheme('light');
        expect(document.documentElement.style.colorScheme).toBe('light');
    });
});
