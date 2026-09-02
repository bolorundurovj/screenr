import {TestBed} from '@angular/core/testing';
import {Router} from '@angular/router';
import {beforeEach, describe, expect, it, vi} from 'vitest';
import {Library} from './library';
import {TauriService} from '../../core/tauri.service';
import {Take} from '../../core/models';

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

function take(over: Partial<Take> = {}): Take {
    return {
        name: 'ScreenR-1.mp4',
        absolutePath: 'C:/Videos/ScreenR/ScreenR-1.mp4',
        size: 10 * MB,
        modifiedTime: 1_700_000_000,
        hasSrt: false,
        ...over,
    };
}

function createLibrary(takes: Take[] = []) {
    const invoke = vi.fn((cmd: string) =>
        cmd === 'get_takes' ? Promise.resolve(takes) : Promise.resolve(undefined),
    );

    TestBed.resetTestingModule();
    TestBed.configureTestingModule({
        providers: [
            {provide: TauriService, useValue: {invoke, listen: vi.fn()}},
            {provide: Router, useValue: {navigate: vi.fn()}},
        ],
    });

    return {library: TestBed.runInInjectionContext(() => new Library()), invoke};
}

describe('Library', () => {
    describe('size formatting', () => {
        let library: Library;

        beforeEach(() => {
            library = createLibrary().library;
        });

        it('uses KB below a megabyte', () => {
            expect(library.formatSize(512 * KB)).toBe('512 KB');
        });

        it('uses MB with one decimal', () => {
            expect(library.formatSize(184 * MB)).toBe('184.0 MB');
        });

        it('switches to GB for large takes', () => {
            expect(library.formatSize(2.5 * GB)).toBe('2.5 GB');
        });

        it('handles an empty file', () => {
            expect(library.formatSize(0)).toBe('0 KB');
        });
    });

    describe('summary line', () => {
        it('reports an empty library', async () => {
            const {library} = createLibrary([]);
            await library.load();
            expect(library.summary()).toBe('No recordings yet');
        });

        it('uses the singular for one take', async () => {
            const {library} = createLibrary([take({size: 5 * MB})]);
            await library.load();
            expect(library.summary()).toBe('1 take · 5.0 MB');
        });

        it('totals the size across takes', async () => {
            const {library} = createLibrary([
                take({absolutePath: 'a', size: 10 * MB}),
                take({absolutePath: 'b', size: 15 * MB}),
            ]);
            await library.load();
            expect(library.summary()).toBe('2 takes · 25.0 MB');
        });
    });

    describe('deleting', () => {
        it('asks for confirmation before the first delete', async () => {
            const item = take();
            const {library, invoke} = createLibrary([item]);
            await library.load();

            await library.confirmDelete(item);

            expect(library.pendingDelete()).toBe(item.absolutePath);
            expect(invoke).not.toHaveBeenCalledWith('delete_take', expect.anything());
        });

        it('deletes on the second call and drops the row', async () => {
            const item = take();
            const {library, invoke} = createLibrary([item]);
            await library.load();

            await library.confirmDelete(item);
            await library.confirmDelete(item);

            expect(invoke).toHaveBeenCalledWith('delete_take', {path: item.absolutePath});
            expect(library.takes()).toHaveLength(0);
            expect(library.pendingDelete()).toBeNull();
        });

        it('can be backed out of', async () => {
            const item = take();
            const {library, invoke} = createLibrary([item]);
            await library.load();

            await library.confirmDelete(item);
            library.cancelDelete();

            expect(library.pendingDelete()).toBeNull();
            expect(invoke).not.toHaveBeenCalledWith('delete_take', expect.anything());
            expect(library.takes()).toHaveLength(1);
        });

        it('only removes the take that was confirmed', async () => {
            const first = take({absolutePath: 'a', name: 'first.mp4'});
            const second = take({absolutePath: 'b', name: 'second.mp4'});
            const {library} = createLibrary([first, second]);
            await library.load();

            await library.confirmDelete(first);
            await library.confirmDelete(first);

            expect(library.takes().map((t) => t.name)).toEqual(['second.mp4']);
        });
    });

    it('clears the loading flag once the listing resolves', async () => {
        const {library} = createLibrary([take()]);
        expect(library.loading()).toBe(true);

        await library.load();

        expect(library.loading()).toBe(false);
    });
});
