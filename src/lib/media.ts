export interface MediaFormat {
  id: string;
  label: string;
  container: string;
  width: number | null;
  height: number | null;
  filesizeBytes: number | null;
  hasVideo: boolean;
  hasAudio: boolean;
}

export interface MediaInfo {
  sourceUrl: string;
  title: string;
  site: string;
  thumbnailUrl: string | null;
  durationSeconds: number | null;
  formats: MediaFormat[];
}

export interface InspectFailure {
  code: string;
  message: string;
}

export interface DownloadTask {
  id: string;
  sourceUrl: string;
  mediaTitle: string;
  formatId: string;
  destinationPath: string;
  outputExtension: string;
  hasVideo: boolean;
  hasAudio: boolean;
}

export interface DownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null;
  speedBytesPerSecond: number | null;
  etaSeconds: number | null;
}

export interface DownloadFailure {
  code: string;
  message: string;
}

export type DownloadEvent = {
  taskId: string;
  sequence: number;
} & (
  | { type: "queued" | "started" | "processing" | "completed" | "cancelled" }
  | { type: "progress"; progress: DownloadProgress }
  | { type: "failed"; error: DownloadFailure }
);
