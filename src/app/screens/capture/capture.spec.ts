import {TestBed} from '@angular/core/testing';
import {Router} from '@angular/router';
import {beforeEach, describe, expect, it, vi} from 'vitest';
import {Capture} from './capture';
import {TauriService} from '../../core/tauri.service';
import {RecordingService} from '../../core/recording.service';
import {SettingsService} from '../../core/settings.service';
import {VideoSource} from '../../core/models';

function source(over: Partial<VideoSource> = {}): VideoSource {
    return {
        id: 'monitor:1',
        name: 'Display 1',
        thumbnail: '',
        width: 2560,
        height: 1440,
        app: null,
        isPrimary: false,
        ...over,
    };
}

const DISPLAYS = [
    source({id: 'monitor:1', name: 'Display 1', isPrimary: true}),
    source({id: 'monitor:2', name: 'Display 2', width: 1920, height: 1080}),
];

const WINDOWS = [
    source({id: 'window:10', name: 'Terminal', app: 'WindowsTerminal', width: 900, height: 600}),
    source({id: 'window:11', name: 'Browser', app: 'chrome', width: 1200, height: 800}),
];

function createCapture() {
    const recording = {start: vi.fn(), active: () => false, stop: vi.fn()};
    const tauri = {
        invoke: vi.fn((cmd: string) => Promise.resolve(cmd === 'get_displays' ? DISPLAYS : WINDOWS)),
        listen: vi.fn(),
    };

    TestBed.resetTestingModule();
    TestBed.configureTestingModule({
        providers: [
            {provide: TauriService, useValue: tauri},
            {provide: RecordingService, useValue: recording},
            {provide: SettingsService, useValue: {settings: () => null, save: vi.fn()}},
            {provide: Router, useValue: {navigate: vi.fn()}},
        ],
    });

    const capture = TestBed.runInInjectionContext(() => new Capture());
    return {capture, recording, tauri};
}

describe('Capture', () => {
    let capture: Capture;
    let recording: {start: ReturnType<typeof vi.fn>};

    beforeEach(async () => {
        const built = createCapture();
        capture = built.capture;
        recording = built.recording;
        await capture.ngOnInit();
    });

    describe('source captions', () => {
        it('labels the primary display', () => {
            expect(capture.describe(DISPLAYS[0])).toBe('2560×1440 · Primary');
        });

        it('labels a secondary display with just its size', () => {
            expect(capture.describe(DISPLAYS[1])).toBe('1920×1080');
        });

        it('labels a window with its owning application', () => {
            expect(capture.describe(WINDOWS[0])).toBe('900×600 · WindowsTerminal');
        });

        it('omits an unknown size rather than printing zeros', () => {
            expect(capture.describe(source({width: 0, height: 0, app: 'Notes'}))).toBe('Notes');
        });
    });

    describe('picker selection', () => {
        it('starts with nothing selected', () => {
            capture.openPicker('display');
            expect(capture.selectedCount()).toBe(0);
        });

        it('toggles a source on and off', () => {
            capture.openPicker('display');

            capture.toggleSelection(DISPLAYS[0]);
            expect(capture.isSelected(DISPLAYS[0])).toBe(true);

            capture.toggleSelection(DISPLAYS[0]);
            expect(capture.isSelected(DISPLAYS[0])).toBe(false);
        });

        it('selects every display at once', () => {
            capture.openPicker('display');
            capture.selectAllDisplays();
            expect(capture.selectedCount()).toBe(DISPLAYS.length);
        });

        it('clears the selection when reopening the picker', () => {
            capture.openPicker('display');
            capture.toggleSelection(DISPLAYS[0]);

            capture.openPicker('window');

            expect(capture.selectedCount()).toBe(0);
        });

        it('clears the selection on cancel', () => {
            capture.openPicker('display');
            capture.toggleSelection(DISPLAYS[0]);

            capture.closePicker();

            expect(capture.picker()).toBeNull();
            expect(capture.selectedCount()).toBe(0);
        });
    });

    describe('confirm label', () => {
        it('is singular for one display', () => {
            capture.openPicker('display');
            capture.toggleSelection(DISPLAYS[0]);
            expect(capture.confirmLabel()).toBe('Record display');
        });

        it('counts multiple displays', () => {
            capture.openPicker('display');
            capture.selectAllDisplays();
            expect(capture.confirmLabel()).toBe('Record 2 displays');
        });

        it('switches wording for windows', () => {
            capture.openPicker('window');
            capture.toggleSelection(WINDOWS[0]);
            expect(capture.confirmLabel()).toBe('Record window');

            capture.toggleSelection(WINDOWS[1]);
            expect(capture.confirmLabel()).toBe('Record 2 windows');
        });
    });

    describe('confirming', () => {
        it('starts a take with every selected source', () => {
            capture.openPicker('display');
            capture.selectAllDisplays();

            capture.confirm();

            expect(recording.start).toHaveBeenCalledWith(DISPLAYS);
            expect(capture.picker()).toBeNull();
        });

        it('does nothing when nothing is ticked', () => {
            capture.openPicker('display');
            capture.confirm();

            expect(recording.start).not.toHaveBeenCalled();
            expect(capture.picker()).toBe('display');
        });

        it('only submits sources from the open picker', () => {
            capture.openPicker('window');
            capture.toggleSelection(WINDOWS[1]);

            capture.confirm();

            expect(recording.start).toHaveBeenCalledWith([WINDOWS[1]]);
        });
    });

    describe('escape', () => {
        it('closes an open picker', () => {
            capture.openPicker('display');
            capture.onEscape();
            expect(capture.picker()).toBeNull();
        });
    });
});
