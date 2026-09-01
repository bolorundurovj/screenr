import {inject, Injectable} from '@angular/core';
import {NavigationEnd, Router} from '@angular/router';
import {filter} from 'rxjs';
import {getCurrentWindow, LogicalSize} from '@tauri-apps/api/window';

interface ScreenChrome {
    title: string;
    width: number;
    height: number;
}

/**
 * Native window title and size per route.
 *
 * The mockup draws each screen as a floating card with its own fake titlebar.
 * The real app is the window, so those card widths become window widths and the
 * screen name goes in the OS title bar.
 */
const CHROME: Record<string, ScreenChrome> = {
    capture: {title: 'ScreenR', width: 420, height: 620},
    settings: {title: 'ScreenR - Settings', width: 700, height: 640},
    library: {title: 'ScreenR - Library', width: 700, height: 640},
    review: {title: 'ScreenR - Review', width: 620, height: 700},
};

@Injectable({providedIn: 'root'})
export class WindowChromeService {
    private router = inject(Router);

    /** No-op outside the main window. */
    start(): void {
        const window = getCurrentWindow();
        if (window.label !== 'main') {
            return;
        }

        this.router.events
            .pipe(filter((event): event is NavigationEnd => event instanceof NavigationEnd))
            .subscribe((event) => {
                const segment = event.urlAfterRedirects.split('?')[0].replace(/^\//, '');
                const chrome = CHROME[segment];
                if (!chrome) {
                    return;
                }
                void window.setTitle(chrome.title);
                void window.setSize(new LogicalSize(chrome.width, chrome.height));
            });
    }
}
