import {
    ApplicationConfig,
    inject,
    provideAppInitializer,
    provideBrowserGlobalErrorListeners,
    provideZoneChangeDetection,
} from '@angular/core';
import {provideRouter, withHashLocation} from '@angular/router';

import {routes} from './app.routes';
import {SettingsService} from './core/settings.service';

export const appConfig: ApplicationConfig = {
    providers: [
        provideBrowserGlobalErrorListeners(),
        provideZoneChangeDetection({eventCoalescing: true}),
        // Hash location so the overlay window can open '#/overlay' directly.
        provideRouter(routes, withHashLocation()),
        provideAppInitializer(() => inject(SettingsService).load()),
    ],
};
