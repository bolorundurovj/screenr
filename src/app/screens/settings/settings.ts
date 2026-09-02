import {Component, inject, OnInit, signal} from '@angular/core';
import {FormsModule} from '@angular/forms';
import {Router} from '@angular/router';
import {open} from '@tauri-apps/plugin-dialog';
import {getVersion} from '@tauri-apps/api/app';
import {SettingsService} from '../../core/settings.service';
import {ToastService} from '../../core/toast.service';
import {describeError} from '../../core/recording.service';
import {WindowFrame} from '../../shared/window-frame/window-frame';
import {ToggleSwitch} from '../../shared/toggle-switch/toggle-switch';
import {ChipButton} from '../../shared/chip-button/chip-button';
import {Settings, Theme, VideoFormat} from '../../core/models';

type TabId = 'capture' | 'audio' | 'drawing' | 'ai' | 'output' | 'appearance' | 'about';

@Component({
    selector: 'app-settings',
    imports: [FormsModule, WindowFrame, ToggleSwitch, ChipButton],
    templateUrl: './settings.html',
})
export class SettingsComponent implements OnInit {
    private settingsService = inject(SettingsService);
    private toast = inject(ToastService);
    readonly router = inject(Router);

    readonly tabs: readonly {id: TabId; label: string}[] = [
        {id: 'capture', label: 'Capture'},
        {id: 'audio', label: 'Audio'},
        {id: 'drawing', label: 'Drawing'},
        {id: 'ai', label: 'AI'},
        {id: 'output', label: 'Output'},
        {id: 'appearance', label: 'Appearance'},
        {id: 'about', label: 'About'},
    ];

    readonly frameRates = [30, 60, 120];
    readonly resolutions = [
        {value: 'source', label: 'Match source'},
        {value: '1080p', label: '1080p'},
        {value: '720p', label: '720p'},
    ] as const;
    readonly formats: readonly {value: VideoFormat; label: string}[] = [
        {value: 'mp4', label: 'MP4 (H.264)'},
        {value: 'webm', label: 'WebM (VP9)'},
        {value: 'mkv', label: 'MKV'},
    ];
    readonly penColors = [
        {value: '#e5484d', label: 'Signal red'},
        {value: '#2f5fd8', label: 'Accent blue'},
        {value: '#f5b912', label: 'Highlight'},
        {value: '#ffffff', label: 'White'},
    ];
    readonly themes: readonly {value: Theme; label: string; note: string}[] = [
        {value: 'light', label: 'Light', note: 'Always light'},
        {value: 'dark', label: 'Dark', note: 'Always dark'},
        {value: 'system', label: 'System', note: 'Follows the OS'},
    ];
    readonly engines = [
        {
            label: 'Ollama',
            endpoint: 'http://localhost:11434',
            model: 'whisper-large-v3-turbo',
            note: 'Talks to a local Ollama daemon. Models are pulled with ollama pull.',
        },
        {
            label: 'LM Studio',
            endpoint: 'http://localhost:1234/v1',
            model: 'whisper-large-v3',
            note: "Uses LM Studio's local server. Start the server from its Developer tab first.",
        },
        {
            label: 'OpenAI-compatible',
            endpoint: 'https://api.example.com/v1',
            model: 'whisper-1',
            note: 'Any endpoint implementing the OpenAI audio/transcriptions spec. Audio leaves the machine.',
        },
    ];

    /** Reported by Tauri; 'unknown' if the call fails. */
    readonly version = signal('…');

    readonly activeTab = signal<TabId>('capture');
    /** Working copy; committed to the backend on Done. */
    readonly draft = signal<Settings | null>(null);

    async ngOnInit(): Promise<void> {
        if (!this.settingsService.loaded()) {
            await this.settingsService.load();
        }
        const current = this.settingsService.settings();
        if (current) {
            this.draft.set({...current});
        }

        await getVersion()
            .then((version) => this.version.set(version))
            .catch(() => this.version.set('unknown'));
    }

    update<K extends keyof Settings>(key: K, value: Settings[K]): void {
        this.draft.update((draft) => (draft ? {...draft, [key]: value} : draft));
    }

    selectEngine(engine: (typeof this.engines)[number]): void {
        this.draft.update((draft) =>
            draft
                ? {
                      ...draft,
                      aiEngine: engine.label,
                      aiEndpoint: engine.endpoint,
                      aiModel: engine.model,
                  }
                : draft,
        );
    }

    get engineNote(): string {
        const active = this.draft()?.aiEngine;
        return this.engines.find((engine) => engine.label === active)?.note ?? '';
    }

    async chooseFolder(): Promise<void> {
        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: this.draft()?.saveFolder || undefined,
        });
        if (typeof selected === 'string') {
            this.update('saveFolder', selected);
        }
    }

    async save(): Promise<void> {
        const draft = this.draft();
        if (!draft) {
            return;
        }
        try {
            await this.settingsService.save(draft);
            await this.router.navigate(['/capture']);
        } catch (e) {
            this.toast.show(describeError(e, 'Could not save settings'), 'error');
        }
    }

    notImplemented(): void {
        this.toast.show('Not implemented yet');
    }
}
