import {TestBed} from '@angular/core/testing';
import {ActivatedRoute, Router} from '@angular/router';
import {beforeEach, describe, expect, it, vi} from 'vitest';
import {Review} from './review';
import {TauriService} from '../../core/tauri.service';

function routeWith(path: string | null) {
    return {
        snapshot: {queryParamMap: {get: () => path}},
    };
}

/**
 * Builds the component without rendering it. The template is not needed: the
 * trim maths guards every video access with optional chaining.
 */
function createReview(path: string | null = 'C:/takes/ScreenR-1.mp4') {
    TestBed.resetTestingModule();
    TestBed.configureTestingModule({
        providers: [
            {provide: ActivatedRoute, useValue: routeWith(path)},
            {provide: Router, useValue: {navigate: vi.fn()}},
            {provide: TauriService, useValue: {invoke: vi.fn(), listen: vi.fn()}},
        ],
    });
    return TestBed.runInInjectionContext(() => new Review());
}

describe('Review', () => {
    let review: Review;

    beforeEach(() => {
        review = createReview();
        review.ngOnInit();
    });

    describe('formatting', () => {
        it('renders minutes and padded seconds', () => {
            expect(review.format(0)).toBe('0:00');
            expect(review.format(9)).toBe('0:09');
            expect(review.format(75)).toBe('1:15');
            expect(review.format(600)).toBe('10:00');
        });

        it('truncates fractional seconds rather than rounding up', () => {
            expect(review.format(59.9)).toBe('0:59');
        });

        it('survives a duration the browser has not resolved yet', () => {
            expect(review.format(NaN)).toBe('0:00');
            expect(review.format(Infinity)).toBe('0:00');
        });
    });

    describe('file name', () => {
        it('takes the last segment of a Windows path', () => {
            expect(review.fileName).toBe('ScreenR-1.mp4');
        });

        it('handles a POSIX path too', () => {
            const posix = createReview('/home/me/Videos/ScreenR/take.mp4');
            posix.ngOnInit();
            expect(posix.fileName).toBe('take.mp4');
        });
    });

    describe('trim range', () => {
        beforeEach(() => {
            review.duration.set(60);
            review.trimStart.set(0);
            review.trimEnd.set(60);
        });

        it('moves the start handle', () => {
            review.setTrimStart('10');
            expect(review.trimStart()).toBe(10);
        });

        it('moves the end handle', () => {
            review.setTrimEnd('45');
            expect(review.trimEnd()).toBe(45);
        });

        it('never lets the start pass the end', () => {
            review.setTrimStart('90');
            expect(review.trimStart()).toBeLessThan(review.trimEnd());
        });

        it('never lets the end pass the start', () => {
            review.trimStart.set(30);
            review.setTrimEnd('5');
            expect(review.trimEnd()).toBeGreaterThan(review.trimStart());
        });

        it('keeps at least half a second selected', () => {
            review.setTrimStart('90');
            expect(review.trimEnd() - review.trimStart()).toBeGreaterThanOrEqual(0.5);
        });

        it('clamps the start at zero', () => {
            review.setTrimStart('-30');
            expect(review.trimStart()).toBe(0);
        });

        it('clamps the end at the duration', () => {
            review.setTrimEnd('900');
            expect(review.trimEnd()).toBe(60);
        });
    });

    describe('playback errors', () => {
        it('explains that the window cannot decode Matroska', () => {
            const mkv = createReview('C:/takes/take.mkv');
            mkv.ngOnInit();
            mkv.onLoadError();

            expect(mkv.loadError()).toContain('.mkv');
            expect(mkv.loadError()).toContain('still saved');
        });

        it('falls back to a generic message for other failures', () => {
            review.onLoadError();
            expect(review.loadError()).toBe('Could not load this take for playback.');
        });
    });

    it('redirects to the library when opened without a take', () => {
        const router = {navigate: vi.fn()};
        TestBed.resetTestingModule();
        TestBed.configureTestingModule({
            providers: [
                {provide: ActivatedRoute, useValue: routeWith(null)},
                {provide: Router, useValue: router},
                {provide: TauriService, useValue: {invoke: vi.fn(), listen: vi.fn()}},
            ],
        });
        const orphan = TestBed.runInInjectionContext(() => new Review());
        orphan.ngOnInit();

        expect(router.navigate).toHaveBeenCalledWith(['/library']);
    });
});
