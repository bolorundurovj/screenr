import {Component, EventEmitter, Input, Output} from '@angular/core';

@Component({
    selector: 'app-chip-button',
    imports: [],
    templateUrl: './chip-button.html',
})
export class ChipButton {
    @Input() label?: string;
    /** Optional swatch shown before the label, e.g. for pen colours. */
    @Input() color?: string;
    @Input() selected = false;
    @Input() disabled = false;
    @Output() clicked = new EventEmitter<void>();
}
