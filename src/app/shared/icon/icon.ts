import {Component, Input} from '@angular/core';

export type IconName =
    | 'library'
    | 'settings'
    | 'pen'
    | 'highlighter'
    | 'clear'
    | 'keystrokes'
    | 'folder'
    | 'delete'
    | 'play'
    | 'pause'
    | 'stop'
    | 'display'
    | 'window'
    | 'copy'
    | 'download'
    | 'srt'
    | 'close'
    | 'check';

@Component({
    selector: 'app-icon',
    template: `
        <svg
            [class]="cssClass"
            [style.width.px]="size"
            [style.height.px]="size"
            aria-hidden="true"
            focusable="false"
        >
            <use [attr.href]="'/assets/icons.svg#' + name"></use>
        </svg>
    `,
    styles: `
        :host {
            display: inline-flex;
            align-items: center;
            justify-content: center;
        }
    `,
})
export class Icon {
    /** Symbol id defined in src/assets/icons.svg. */
    @Input({required: true}) name!: IconName;
    @Input() size = 16;
    @Input() cssClass = '';
}
