import {Routes} from '@angular/router';

export const routes: Routes = [
    {path: '', redirectTo: 'capture', pathMatch: 'full'},
    {path: 'capture', loadComponent: () => import('./screens/capture/capture').then((m) => m.Capture)},
    {
        path: 'settings',
        loadComponent: () => import('./screens/settings/settings').then((m) => m.SettingsComponent),
    },
    {path: 'review', loadComponent: () => import('./screens/review/review').then((m) => m.Review)},
    {path: 'library', loadComponent: () => import('./screens/library/library').then((m) => m.Library)},
    {path: 'overlay', loadComponent: () => import('./screens/overlay/overlay').then((m) => m.Overlay)},
];
