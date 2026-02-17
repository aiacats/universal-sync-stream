// Server configuration
export interface ServerConfig {
  server_url: string;
  api_key: string | null;
  token: string | null;
}

// Health check response
export interface HealthResponse {
  status: string;
  version: string;
}

// Token response
export interface TokenResponse {
  token: string;
  expires_in: number;
}

// Room information
export interface RoomResponse {
  id: string;
  state: string;
  participant_count: number;
  server_id: string;
}

// Room statistics
export interface RoomStats {
  id: string;
  state: string;
  participant_count: number;
  receiver_count: number;
  has_primary_sender: boolean;
  has_backup_sender: boolean;
  packets_forwarded: number;
  bytes_forwarded: number;
  uptime_secs: number;
}

// Participant information
export interface ParticipantResponse {
  id: string;
  addr: string;
  role: string;
  state: string;
  session_id: number;
}

// Server information
export interface ServerResponse {
  id: string;
  media_addr: string;
  control_addr: string;
  load: number;
  healthy: boolean;
  room_count: number;
  participant_count: number;
}

// Global statistics
export interface GlobalStatsResponse {
  total_servers: number;
  total_rooms: number;
  total_participants: number;
  total_packets_forwarded: number;
  total_bytes_forwarded: number;
}

// Sidecar server configuration
export interface SidecarConfig {
  server_id: string;
  media_addr: string;
  control_addr: string;
  no_auth: boolean;
  max_rooms: number;
  max_participants: number;
  log_level: string;
}

// Navigation tabs
export type Tab = "dashboard" | "rooms" | "servers" | "settings";
