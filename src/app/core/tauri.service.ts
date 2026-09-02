import {Injectable} from '@angular/core';
import {invoke} from '@tauri-apps/api/core';
import {EventCallback, listen, UnlistenFn} from '@tauri-apps/api/event';

@Injectable({providedIn: 'root'})
export class TauriService {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        return invoke<T>(cmd, args);
    }

    listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
        return listen<T>(event, handler);
    }
}
