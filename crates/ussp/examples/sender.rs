//! Example USSP sender.
//!
//! Usage: cargo run --example sender -- --dest 127.0.0.1:5001

use bytes::Bytes;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use ussp::protocol::{AudioCodecType, AudioPayload, SyncPoint, VideoCodecType, VideoPayload};
use ussp::transport::{Sender, SenderConfig};

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
        .unwrap_or("0.0.0.0:5000");

    let dest_addr = args
        .iter()
        .position(|a| a == "--dest")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1:5001");

    info!("Starting USSP sender");
    info!("  Local address: {}", local_addr);
    info!("  Destination: {}", dest_addr);

    // Create sender with default configuration
    let config = SenderConfig::default();
    let sender = Sender::bind(local_addr, dest_addr, config).await?;

    // Initialize session
    sender.init_session().await?;
    info!("Session initialized");

    // Give receiver time to process
    sleep(Duration::from_millis(100)).await;

    // Simulate streaming at 30 fps video + 48kHz audio
    let mut video_interval = interval(Duration::from_millis(33)); // ~30 fps
    let mut audio_interval = interval(Duration::from_millis(20)); // 20ms audio frames
    let mut sync_interval = interval(Duration::from_secs(1));

    let mut video_pts: u64 = 0;
    let mut audio_pts: u64 = 0;
    let mut frame_count: u64 = 0;

    info!("Starting media stream...");

    loop {
        tokio::select! {
            _ = video_interval.tick() => {
                // Generate dummy video frame
                let frame_id = sender.next_frame_id().await;
                let is_key_frame = frame_count % 30 == 0; // Key frame every 30 frames

                // Simulate H.264 NAL unit (with start code)
                let mut data = vec![0x00, 0x00, 0x00, 0x01];
                if is_key_frame {
                    data.push(0x67); // SPS NAL type
                } else {
                    data.push(0x61); // Non-IDR slice NAL type
                }
                // Add dummy payload
                data.extend_from_slice(&vec![0u8; 1000]);

                let payload = VideoPayload::new(
                    frame_id,
                    VideoCodecType::H264,
                    is_key_frame,
                    video_pts,
                    video_pts.saturating_sub(33_333), // DTS slightly before PTS
                    Bytes::from(data),
                );

                sender.send_video(payload).await?;

                video_pts += 33_333; // ~30 fps in microseconds
                frame_count += 1;

                if frame_count % 100 == 0 {
                    let stats = sender.stats().await;
                    info!(
                        "Sent {} video frames, {} packets, {} bytes",
                        frame_count, stats.packets_sent, stats.bytes_sent
                    );
                }
            }

            _ = audio_interval.tick() => {
                // Send audio for each track (2 tracks: stereo left/right or different sources)
                for track_id in 0..2u16 {
                    // Simulate Opus audio frame (20ms @ 48kHz = 960 samples)
                    let data = vec![0u8; 120]; // Typical Opus frame size

                    let payload = AudioPayload::new(
                        track_id,
                        48000,
                        2,
                        16,
                        AudioCodecType::Opus,
                        audio_pts,
                        960,
                        Bytes::from(data),
                    );

                    sender.send_audio(payload).await?;
                }

                audio_pts += 20_000; // 20ms in microseconds
            }

            _ = sync_interval.tick() => {
                // Send sync point
                let mut sync = SyncPoint::new(
                    ussp::sync::ClockSync::current_time_us(),
                    video_pts,
                );
                sync.add_track(0, audio_pts, 0);
                sync.add_track(1, audio_pts, 0);

                sender.send_sync_point(sync).await?;
                info!("Sync point sent at video_pts={}, audio_pts={}", video_pts, audio_pts);
            }
        }

        // Stop after 10 seconds for demo
        if frame_count >= 300 {
            break;
        }
    }

    // Cleanup
    sender.close().await?;
    info!("Sender closed");

    Ok(())
}
