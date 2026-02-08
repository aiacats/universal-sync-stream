// Device information
export interface VideoDeviceInfo {
  index: number;
  name: string;
  description: string;
}

export interface AudioDeviceInfo {
  index: number;
  name: string;
  sample_rate: number;
  channels: number;
}

// Resolution options
export type Resolution = "640x480" | "1280x720" | "1920x1080";

// Video source
export type VideoSource =
  | { type: "test-pattern" }
  | { type: "camera"; device_name: string };

// Audio source
export type AudioSource =
  | { type: "test-tone" }
  | { type: "microphone"; device_index: number };

// Sender options
export interface SenderOptions {
  audio_tracks: number;
  fec_enabled: boolean;
  fps: number;
  resolution: Resolution;
  video_source: VideoSource;
  audio_source: AudioSource;
}

export interface SenderStats {
  packets_sent: number;
  bytes_sent: number;
  video_frames_sent: number;
  audio_frames_sent: number;
  fps: number;
}

export interface ReceiverStats {
  packets_received: number;
  bytes_received: number;
  video_frames_received: number;
  audio_frames_received: number;
  packet_loss_rate: number;
  av_sync_diff_us: number;
  fps: number;
}

export interface VideoFrameData {
  width: number;
  height: number;
  pts: number;
  is_key_frame: boolean;
  data_base64: string;
}

export interface AudioLevelData {
  track_id: number;
  level_db: number;
  pts: number;
  samples: number[]; // Normalized samples (-1.0 to 1.0)
}

export interface SyncPointData {
  video_pts: number;
  audio_pts: number[];
  av_diff_us: number;
}

// Camera preview data
export interface CameraPreviewData {
  width: number;
  height: number;
  data: string; // Base64 encoded RGB data
}
