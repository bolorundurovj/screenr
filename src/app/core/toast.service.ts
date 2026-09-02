import {DestroyRef, inject, Injectable, signal} from '@angular/core';

export type ToastType = 'neutral' | 'success' | 'error';

export interface Toast {
    message: string;
    type: ToastType;
}

const DEFAULT_DURATION_MS = 2400;

@Injectable({providedIn: 'root'})
export class ToastService {
    private readonly current = signal<Toast | null>(null);
    private timeout?: ReturnType<typeof setTimeout>;

    readonly active = this.current.asReadonly();

    constructor() {
        inject(DestroyRef).onDestroy(() => this.clearTimeout());
    }

    show(message: string, type: ToastType = 'neutral', duration = DEFAULT_DURATION_MS): void {
        this.clearTimeout();
        this.current.set({message, type});
        this.timeout = setTimeout(() => this.current.set(null), duration);
    }

    dismiss(): void {
        this.clearTimeout();
        this.current.set(null);
    }

    private clearTimeout(): void {
        if (this.timeout !== undefined) {
            clearTimeout(this.timeout);
            this.timeout = undefined;
        }
    }
}
