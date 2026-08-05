import { Component, ElementRef, OnInit, ViewChild } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

const NOTIFICATION_TITLE = "ScreenR";
const NOTIFICATION_BODY = "ScreenR is ready to go";
const MIME_TYPE = "video/webm; codecs=vp9";

@Component({
  selector: "app-root",
  imports: [],
  templateUrl: "./app.component.html",
  styleUrl: "./app.component.css",
})
export class AppComponent implements OnInit {
  @ViewChild("video", { static: true })
  videoElement!: ElementRef<HTMLVideoElement>;

  title = "⚡ ScreenR";
  startLabel = "Start";
  selectLabel = "Choose Video Source";
  isRecording = false;

  private mediaRecorder?: MediaRecorder;
  private recordedChunks: Blob[] = [];

  async ngOnInit(): Promise<void> {
    await this.showNotification();
  }

  // Notify the user that the app is ready, mirroring the original startup toast.
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

  // Prompt the OS screen/window picker and preview the selected source.
  async selectSource(): Promise<void> {
    const stream = await navigator.mediaDevices.getDisplayMedia({
      audio: false,
      video: true,
    });

    const [track] = stream.getVideoTracks();
    this.title = track?.label || "Screen";
    this.selectLabel = "Change Video Source";

    // Preview the source in the video element.
    const video = this.videoElement.nativeElement;
    video.srcObject = stream;
    await video.play();

    // Create the media recorder for the selected stream.
    this.recordedChunks = [];
    this.mediaRecorder = new MediaRecorder(stream, { mimeType: MIME_TYPE });
    this.mediaRecorder.ondataavailable = (event) =>
      this.handleAvailableData(event);
    this.mediaRecorder.onstop = () => this.handleStop();
  }

  startRecording(): void {
    if (!this.mediaRecorder) {
      return;
    }
    this.mediaRecorder.start();
    this.isRecording = true;
    this.startLabel = "Recording";
  }

  stopRecording(): void {
    if (!this.mediaRecorder) {
      return;
    }
    this.mediaRecorder.stop();
    this.isRecording = false;
    this.startLabel = "Start";
  }

  private handleAvailableData(event: BlobEvent): void {
    this.recordedChunks.push(event.data);
  }

  // Save the recorded video once recording stops.
  private async handleStop(): Promise<void> {
    const blob = new Blob(this.recordedChunks, { type: MIME_TYPE });
    const buffer = new Uint8Array(await blob.arrayBuffer());

    const filePath = await save({
      title: "Save Video",
      defaultPath: `vid-${Date.now()}.webm`,
      filters: [{ name: "WebM Video", extensions: ["webm"] }],
    });

    if (filePath) {
      await invoke("save_recording", {
        path: filePath,
        contents: Array.from(buffer),
      });
      console.log("Saved Successfully");
    }
  }
}
