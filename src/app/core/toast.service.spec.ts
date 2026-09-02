import {TestBed} from '@angular/core/testing';
import {afterEach, beforeEach, describe, expect, it, vi} from 'vitest';
import {ToastService} from './toast.service';

describe('ToastService', () => {
    let toast: ToastService;

    beforeEach(() => {
        vi.useFakeTimers();
        TestBed.configureTestingModule({});
        toast = TestBed.inject(ToastService);
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('starts with nothing showing', () => {
        expect(toast.active()).toBeNull();
    });

    it('shows a neutral toast by default', () => {
        toast.show('Take discarded');
        expect(toast.active()).toEqual({message: 'Take discarded', type: 'neutral'});
    });

    it('carries the requested variant', () => {
        toast.show('Export failed', 'error');
        expect(toast.active()?.type).toBe('error');
    });

    it('dismisses itself after the default duration', () => {
        toast.show('MP4 saved');

        vi.advanceTimersByTime(2399);
        expect(toast.active()).not.toBeNull();

        vi.advanceTimersByTime(1);
        expect(toast.active()).toBeNull();
    });

    it('honours a custom duration', () => {
        toast.show('Rendering GIF', 'neutral', 500);

        vi.advanceTimersByTime(500);
        expect(toast.active()).toBeNull();
    });

    it('replaces an existing toast rather than queueing', () => {
        toast.show('first');
        vi.advanceTimersByTime(1000);
        toast.show('second');

        expect(toast.active()?.message).toBe('second');

        // The first toast's timer must not cut the second one short.
        vi.advanceTimersByTime(1400);
        expect(toast.active()?.message).toBe('second');

        vi.advanceTimersByTime(1000);
        expect(toast.active()).toBeNull();
    });

    it('can be dismissed early', () => {
        toast.show('Deleted take');
        toast.dismiss();
        expect(toast.active()).toBeNull();
    });
});
