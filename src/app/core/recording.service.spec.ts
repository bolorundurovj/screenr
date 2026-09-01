import {TestBed} from '@angular/core/testing';
import {Router} from '@angular/router';
import {describe, expect, it, vi} from 'vitest';
import {describeError, RecordingService} from './recording.service';
import {TauriService} from './tauri.service';
import {ToastService} from './toast.service';
import {RecordingState, VideoSource} from './models';

describe('describeError', () => {
    it('passes through a Tauri command rejection', () => {
        expect(describeError('No save folder configured', 'fallback')).toBe('No save folder configured');
    });

    it('falls back when the rejection is empty', () => {
        expect(describeError('', 'Could not start recording')).toBe('Could not start recording');
    });

    it('falls back for non-string throwables', () => {
        expect(describeError(new Error('boom'), 'fallback')).toBe('fallback');
        expect(describeError(undefined, 'fallback')).toBe('fallback');
        expect(describeError({code: 500}, 'fallback')).toBe('fallback');
    });
});

const PRIMARY: VideoSource = {
    id: 'monitor:1',
    name: 'Display 1',
    thumbnail: '',
    width: 2560,
    height: 1440,
    app: null,
    isPrimary: true,
};

const SECONDARY: VideoSource = {...PRIMARY, id: 'monitor:2', name: 'Display 2', isPrimary: false};

function createService(displays: VideoSource[] = [SECONDARY, PRIMARY]) {
    /** Event name -> handler, so tests can push backend events in. */
    const handlers = new Map<string, (event: {payload: unknown}) => void>();

    // Explicit return type: without it the union of resolved shapes stops
    // mockImplementation from accepting a rejection.
    const invoke = vi.fn((cmd: string): Promise<unknown> => {
        if (cmd === 'get_displays') {
            return Promise.resolve(displays);
        }
        return Promise.resolve('C:/Videos/ScreenR/take.mp4');
    });

    const tauri = {
        invoke,
        listen: vi.fn((event: string, handler: (e: {payload: unknown}) => void) => {
            handlers.set(event, handler);
            return Promise.resolve(() => undefined);
        }),
    };

    TestBed.resetTestingModule();
    TestBed.configureTestingModule({
        providers: [
            {provide: TauriService, useValue: tauri},
            {provide: Router, useValue: {navigate: vi.fn()}},
            ToastService,
        ],
    });

    const service = TestBed.inject(RecordingService);
    const emit = (event: string, payload?: unknown) => handlers.get(event)?.({payload});
    return {service, invoke, emit};
}

function recordingState(over: Partial<RecordingState> = {}): RecordingState {
    return {isRecording: true, isPaused: false, elapsedSecs: 0, ...over};
}

describe('RecordingService', () => {
    describe('starting', () => {
        it('passes every selected source to the backend', async () => {
            const {service, invoke} = createService();

            await service.start([PRIMARY, SECONDARY]);

            expect(invoke).toHaveBeenCalledWith('start_recording', {
                sourceIds: ['monitor:1', 'monitor:2'],
            });
        });

        it('opens the control bar', async () => {
            const {service, invoke} = createService();
            await service.start([PRIMARY]);
            expect(invoke).toHaveBeenCalledWith('open_overlay');
        });

        it('ignores an empty selection', async () => {
            const {service, invoke} = createService();
            await service.start([]);
            expect(invoke).not.toHaveBeenCalled();
        });

        it('is active before the capture loop reports in', async () => {
            const {service} = createService();

            await service.start([PRIMARY]);

            // The countdown runs before any recording_state arrives.
            expect(service.state().isRecording).toBe(false);
            expect(service.active()).toBe(true);
        });

        it('refuses a second take while one is starting', async () => {
            const {service, invoke} = createService();

            await service.start([PRIMARY]);
            invoke.mockClear();
            await service.start([SECONDARY]);

            expect(invoke).not.toHaveBeenCalled();
        });

        it('goes idle again when the backend rejects the take', async () => {
            const {service, invoke} = createService();
            invoke.mockImplementation(() => Promise.reject('No save folder configured'));

            await service.start([PRIMARY]);

            expect(service.active()).toBe(false);
        });
    });

    describe('shortcut toggle', () => {
        it('records the primary display when idle', async () => {
            const {service, invoke} = createService();

            await service.toggle();

            expect(invoke).toHaveBeenCalledWith('start_recording', {
                sourceIds: ['monitor:1'],
            });
        });

        it('falls back to the first display when none is primary', async () => {
            const {service, invoke} = createService([SECONDARY]);

            await service.toggle();

            expect(invoke).toHaveBeenCalledWith('start_recording', {
                sourceIds: ['monitor:2'],
            });
        });

        it('stops a take that is already running', async () => {
            const {service, invoke, emit} = createService();
            emit('recording_state', recordingState());
            invoke.mockClear();

            await service.toggle();

            expect(invoke).toHaveBeenCalledWith('stop_recording');
        });

        it('stops a take that is still counting down', async () => {
            const {service, invoke} = createService();
            await service.start([PRIMARY]);
            invoke.mockClear();

            await service.toggle();

            expect(invoke).toHaveBeenCalledWith('stop_recording');
        });

        it('reports when there is no display to record', async () => {
            const {service, invoke} = createService([]);
            const toast = TestBed.inject(ToastService);

            await service.toggle();

            expect(invoke).not.toHaveBeenCalledWith('start_recording', expect.anything());
            expect(toast.active()?.type).toBe('error');
        });
    });

    describe('backend events', () => {
        it('tracks the elapsed time', () => {
            const {service, emit} = createService();

            emit('recording_state', recordingState({elapsedSecs: 42}));

            expect(service.state().elapsedSecs).toBe(42);
        });

        it('exposes the countdown and clears it at zero', () => {
            const {service, emit} = createService();

            emit('countdown_tick', 3);
            expect(service.countdown()).toBe(3);

            emit('countdown_tick', 0);
            expect(service.countdown()).toBeNull();
        });

        it('goes idle when a take is cancelled during the countdown', async () => {
            const {service, emit} = createService();
            await service.start([PRIMARY]);

            // Cancelling before capture starts never reaches recording_finished.
            emit('recording_state', recordingState({isRecording: false}));

            expect(service.active()).toBe(false);
        });
    });
});
