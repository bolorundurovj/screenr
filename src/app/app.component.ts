import {Component, inject} from '@angular/core';
import {RouterOutlet} from '@angular/router';
import {ToastService, ToastType} from './core/toast.service';
import {WindowChromeService} from './core/window-chrome.service';

@Component({
    selector: 'app-root',
    imports: [RouterOutlet],
    templateUrl: './app.component.html',
})
export class AppComponent {
    readonly toast = inject(ToastService);

    constructor() {
        inject(WindowChromeService).start();
    }

    readonly toastStyles: Record<ToastType, string> = {
        neutral: 'bg-surface text-ink border border-border',
        success: 'bg-success text-white',
        error: 'bg-danger text-white',
    };
}
