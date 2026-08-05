import { Component, OnInit, ChangeDetectorRef, NgZone } from "@angular/core";
import { CommonModule } from "@angular/common";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

const NOTIFICATION_TITLE = "ScreenR";
const NOTIFICATION_BODY = "ScreenR is ready to go";

interface VideoSource {
  id: string;
  name: string;
  thumbnail: string;
}

@Component({
  selector: "app-root",
  standalone: true,
  imports: [CommonModule],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent implements OnInit {
  title = "⚡ ScreenR";
  startLabel = "Start";
  selectLabel = "Choose Video Source";
  isRecording = false;

  displays: VideoSource[] = [];
  windows: VideoSource[] = [];
  activeTab: 'displays' | 'windows' = 'displays';
  selectedSourceId: string | null = null;
  previewSrc: string = "";
  showPicker = false;

  constructor(private cdr: ChangeDetectorRef, private ngZone: NgZone) {}

  async ngOnInit(): Promise<void> {
    await this.showNotification();
    
    try {
      await invoke("init_ffmpeg");
      console.log("FFmpeg initialized");
    } catch (e) {
      console.error("Failed to init ffmpeg:", e);
    }

    // Listen for preview frames from Rust
    await listen<string>("preview_frame", (event) => {
      this.ngZone.run(() => {
        this.previewSrc = event.payload;
        this.cdr.detectChanges();
      });
    });
  }

  private async showNotification(): Promise<void> {
    try {
      let granted = await isPermissionGranted();
      if (!granted) {
        const permission = await requestPermission();
        granted = permission === "granted";
      }
      if (granted) {
        sendNotification({ title: NOTIFICATION_TITLE, body: NOTIFICATION_BODY });
      }
    } catch (error) {
      console.error("Failed to show notification", error);
    }
  }

  async fetchSources(): Promise<void> {
    try {
      this.displays = [];
      this.windows = [];
      
      // Fetch displays (fast)
      this.displays = await invoke<VideoSource[]>("get_displays");
      
      this.activeTab = 'displays';
      this.showPicker = true;

      // Fetch windows (slower) asynchronously without blocking UI
      invoke<VideoSource[]>("get_windows").then((wins) => {
        this.ngZone.run(() => {
          this.windows = wins;
          this.cdr.detectChanges();
        });
      }).catch(e => console.error("Failed to load windows", e));
      
    } catch (e) {
      console.error("Failed to fetch displays", e);
    }
  }

  selectSource(source: VideoSource): void {
    this.selectedSourceId = source.id;
    this.title = source.name;
    this.selectLabel = "Change Video Source";
    this.showPicker = false;
    this.previewSrc = source.thumbnail;
  }

  closePicker(): void {
    this.showPicker = false;
  }

  async startRecording(): Promise<void> {
    if (!this.selectedSourceId) return;

    try {
      const filePath = await save({
        title: "Save Video",
        defaultPath: `vid-${Date.now()}.mp4`,
        filters: [{ name: "MP4 Video", extensions: ["mp4"] }],
      });

      if (!filePath) return;

      await invoke("start_recording", {
        sourceId: this.selectedSourceId,
        path: filePath,
      });

      this.isRecording = true;
      this.startLabel = "Recording";
    } catch (e) {
      console.error("Failed to start recording", e);
    }
  }

  async stopRecording(): Promise<void> {
    try {
      await invoke("stop_recording");
      this.isRecording = false;
      this.startLabel = "Start";
      console.log("Saved Successfully");
    } catch (e) {
      console.error("Failed to stop recording", e);
    }
  }
}
