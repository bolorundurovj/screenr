import {Component, computed, inject} from '@angular/core';
import {RecordingService} from '../../core/recording.service';
import {Icon} from '../../shared/icon/icon';

/**
 * Contents of the floating control bar window shown while recording.
 *
 * The window is sized to the bar itself rather than the whole screen, so the
 * desktop underneath stays clickable while it is being captured.
 */
@Component({
    selector: 'app-overlay',
    imports: [Icon],
    templateUrl: './overlay.html',
})
export class Overlay {
    readonly recording = inject(RecordingService);

    readonly elapsed = computed(() => formatElapsed(this.recording.state().elapsedSecs));
}

/** HH:MM:SS, matching the design's monospace timer. */
function formatElapsed(totalSeconds: number): string {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = Math.floor(totalSeconds % 60);
    return [hours, minutes, seconds].map((part) => String(part).padStart(2, '0')).join(':');
}
