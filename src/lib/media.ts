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
}
