export interface VideoSource {
    id: string;
    name: string;
    /** base64 JPEG data URL; empty when the capture failed. */
    thumbnail: string;
    width: number;
    height: number;
    /** Owning application, for windows only. */
    app: string | null;
    isPrimary: boolean;
}

export type Theme = 'light' | 'dark' | 'system';
export type Resolution = 'source' | '1080p' | '720p';
export type VideoFormat = 'mp4' | 'webm' | 'mkv';

export interface Settings {
    fps: number;
    resolution: Resolution;
    showCursor: boolean;
    countdown: boolean;
    saveFolder: string;
    format: VideoFormat;
    mic: boolean;
    systemAudio: boolean;
    drawTools: boolean;
    penColor: string;
    theme: Theme;
    overlayFollowsTheme: boolean;
    srtDefault: boolean;
    aiEngine: string;
    aiEndpoint: string;
    aiModel: string;
    aiLanguage: string;
}

export interface Take {
    name: string;
    absolutePath: string;
    size: number;
    /** Seconds since the Unix epoch. */
    modifiedTime: number;
    hasSrt: boolean;
}

export interface RecordingState {
    isRecording: boolean;
    isPaused: boolean;
    elapsedSecs: number;
}

export interface RecordingFinished {
    path: string;
    durationSecs: number;
}

export const IDLE_RECORDING_STATE: RecordingState = {
    isRecording: false,
    isPaused: false,
    elapsedSecs: 0,
};
