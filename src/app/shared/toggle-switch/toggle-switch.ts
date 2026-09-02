import {Component, forwardRef, Input} from '@angular/core';
import {ControlValueAccessor, NG_VALUE_ACCESSOR} from '@angular/forms';

@Component({
    selector: 'app-toggle-switch',
    imports: [],
    templateUrl: './toggle-switch.html',
    providers: [
        {
            provide: NG_VALUE_ACCESSOR,
            useExisting: forwardRef(() => ToggleSwitch),
            multi: true,
        },
    ],
})
export class ToggleSwitch implements ControlValueAccessor {
    @Input() disabled = false;
    @Input() ariaLabel?: string;

    value = false;

    private onChange: (value: boolean) => void = () => {};
    private onTouched: () => void = () => {};

    writeValue(value: boolean): void {
        this.value = !!value;
    }

    registerOnChange(fn: (value: boolean) => void): void {
        this.onChange = fn;
    }

    registerOnTouched(fn: () => void): void {
        this.onTouched = fn;
    }

    setDisabledState(isDisabled: boolean): void {
        this.disabled = isDisabled;
    }

    toggle() {
        if (!this.disabled) {
            this.value = !this.value;
            this.onChange(this.value);
            this.onTouched();
        }
    }
}
