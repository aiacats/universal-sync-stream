//! Example USSP receiver.
//!
//! Usage: cargo run --example receiver -- --addr 0.0.0.0:5001

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use ussp::sync::{JitterBufferConfig, MultiTrackBuffer};
use ussp::transport::{Receiver, ReceiverConfig, ReceiverEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse arguments
    let args: Vec<String> = std::env::args().collect();
    let local_addr = args
        .iter()
        .position(|a| a == "--addr")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("0.0.0.0:5001");

    info!("Starting USSP receiver on {}", local_addr);

    // Create receiver
    let config = ReceiverConfig::default();
    let receiver = Arc::new(Receiver::bind(local_addr, config).await?);

    // Clone for the background task
    let receiver_clone = Arc::clone(&receiver);

    // Start receiver in background
    let _receiver_handle = tokio::spawn(async move {
        if let Err(e) = receiver_clone.run().await {
            warn!("Receiver error: {}", e);
        }
    });

    // Jitter buffer (will be initialized after session init)
    let mut buffer: Option<MultiTrackBuffer> = None;

    // Statistics
    let mut video_frames = 0u64;
    let mut audio_frames = 0u64;
    let mut dmx_frames = 0u64;
    let mut mocap_frames = 0u64;
    let mut last_video_pts = 0u64;
    let mut last_audio_pts = 0u64;

    info!("Waiting for incoming stream...");

    // Process events
    loop {
        match timeout(Duration::from_secs(30), receiver.recv()).await {
            Ok(Some(event)) => match event {
                ReceiverEvent::SessionInit {
                    session_id,
                    audio_tracks,
                } => {
                    info!(
                        "Session initialized: id={}, audio_tracks={}",
                        session_id, audio_tracks
                    );

                    // Initialize jitter buffer
                    let jb_config = JitterBufferConfig {
                        target_depth_us: 100_000, // 100ms
                        min_depth_us: 50_000,
                        max_depth_us: 500_000,
                        max_frames: 300,
                    };
                    buffer = Some(MultiTrackBuffer::new(jb_config, audio_tracks as usize));
                }

                ReceiverEvent::VideoFrame(frame) => {
                    video_frames += 1;
                    last_video_pts = frame.pts;

                    if let Some(ref mut buf) = buffer {
                        buf.insert_video(frame);
                    }

                    if video_frames % 30 == 0 {
                        info!(
                            "Received {} video frames (last PTS: {} us, key: {})",
                            video_frames,
                            last_video_pts,
                            video_frames % 30 == 1
                        );

                        if let Some(ref buf) = buffer {
                            info!(
                                "  Video buffer: {} frames, depth {} us, state {:?}",
                                buf.video.len(),
                                buf.video.depth_us(),
                                buf.video.state()
                            );
                        }
                    }
                }

                ReceiverEvent::AudioFrame(frame) => {
                    audio_frames += 1;
                    last_audio_pts = frame.pts;

                    if let Some(ref mut buf) = buffer {
                        buf.insert_audio(frame);
                    }

                    if audio_frames % 100 == 0 {
                        info!(
                            "Received {} audio frames (last PTS: {} us)",
                            audio_frames, last_audio_pts
                        );
                    }
                }

                ReceiverEvent::DmxFrame(frame) => {
                    dmx_frames += 1;

                    if dmx_frames % 100 == 0 {
                        info!(
                            "Received {} DMX frames (universe: {}, channels: {})",
                            dmx_frames, frame.universe, frame.data.len()
                        );
                    }
                }

                ReceiverEvent::MocapFrame(frame) => {
                    mocap_frames += 1;

                    if mocap_frames % 120 == 0 {
                        info!(
                            "Received {} mocap frames (frame: {}, rigid_bodies: {}, skeletons: {}, markers: {})",
                            mocap_frames,
                            frame.frame_number,
                            frame.rigid_bodies.len(),
                            frame.skeletons.len(),
                            frame.labeled_markers.len()
                        );
                    }
                }

                ReceiverEvent::SyncPoint(sync) => {
                    info!(
                        "Sync point: ref_time={}, video_pts={}, audio_tracks={}",
                        sync.reference_timestamp,
                        sync.video_pts,
                        sync.audio_tracks.len()
                    );

                    for track in &sync.audio_tracks {
                        info!(
                            "  Track {}: pts={}, offset={}us",
                            track.track_id, track.pts, track.offset_us
                        );
                    }

                    // Calculate A/V sync
                    let av_diff = last_video_pts as i64 - last_audio_pts as i64;
                    info!("  A/V sync difference: {} us", av_diff);
                }

                ReceiverEvent::Disconnected => {
                    warn!("Disconnected from sender");
                    break;
                }
            },

            Ok(None) => {
                info!("Event channel closed");
                break;
            }

            Err(_) => {
                warn!("Timeout waiting for data");
                break;
            }
        }

        // Exit after receiving enough frames
        if video_frames >= 300 {
            break;
        }
    }

    // Print final statistics
    info!("=== Final Statistics ===");
    info!("Video frames received: {}", video_frames);
    info!("Audio frames received: {}", audio_frames);
    info!("DMX frames received: {}", dmx_frames);
    info!("Mocap frames received: {}", mocap_frames);

    if let Some(stats) = receiver.stats() {
        info!("Packets received: {}", stats.packets_received);
        info!("Bytes received: {}", stats.bytes_received);
    }

    if let Some(ref buf) = buffer {
        let vstats = buf.video.stats();
        info!("Video buffer stats:");
        info!("  Frames received: {}", vstats.frames_received);
        info!("  Frames output: {}", vstats.frames_output);
        info!("  Frames dropped (late): {}", vstats.frames_dropped_late);
        info!("  Buffer underruns: {}", vstats.underruns);
    }

    Ok(())
}
