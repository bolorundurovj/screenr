import {Component, EventEmitter, Input, Output} from '@angular/core';

/**
 * Chrome shared by the in-window screens: a heading row and a scrollable body.
 *
 * This is not a simulated OS titlebar; Tauri supplies the real one. The mockup's
 * fake window card and its centred app name are deliberately absent.
 */
@Component({
    selector: 'app-window-frame',
    templateUrl: './window-frame.html',
})
export class WindowFrame {
    @Input() title = '';
    @Input() subtitle = '';
    @Input() showDone = false;
    @Input() doneLabel = 'Done';
    @Output() done = new EventEmitter<void>();
}
