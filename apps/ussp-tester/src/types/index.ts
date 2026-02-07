export interface SenderOptions {
  audio_tracks: number;
  fec_enabled: boolean;
  fps: number;
  width: number;
  height: number;
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
}

export interface SyncPointData {
  video_pts: number;
  audio_pts: number[];
  av_diff_us: number;
}
