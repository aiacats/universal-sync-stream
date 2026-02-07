//! Integration tests for USSP.

use bytes::Bytes;
use ussp::fec::{DecodeResult, FecConfig, FecDecoder, FecEncoder};
use ussp::protocol::{
    AudioCodecType, AudioPayload, DmxPayload, Packet, PacketHeader, PacketType, Payload,
    SyncPoint, VideoCodecType, VideoPayload,
};
use ussp::sync::{BufferState, JitterBuffer, JitterBufferConfig};

#[test]
fn test_video_packet_encode_decode() {
    let payload = VideoPayload::new(
        1,
        VideoCodecType::H264,
        true,
        1_000_000,
        900_000,
        Bytes::from(vec![0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f]),
    );

    let packet = Packet {
        header: PacketHeader::new(PacketType::VideoFrame, 12345, 1, 1_000_000),
        payload: Payload::Video(payload),
    };

    let encoded = packet.encode().unwrap();
    let decoded = Packet::decode(encoded).unwrap();

    assert_eq!(decoded.header.session_id, 12345);
    assert_eq!(decoded.header.packet_type, PacketType::VideoFrame);

    if let Payload::Video(v) = decoded.payload {
        assert_eq!(v.frame_id, 1);
        assert_eq!(v.codec, VideoCodecType::H264);
        assert!(v.is_key_frame);
        assert_eq!(v.pts, 1_000_000);
    } else {
        panic!("Expected video payload");
    }
}

#[test]
fn test_audio_packet_encode_decode() {
    let payload = AudioPayload::new(
        0,
        48000,
        2,
        16,
        AudioCodecType::Opus,
        1_000_000,
        960,
        Bytes::from(vec![0u8; 120]),
    );

    let packet = Packet {
        header: PacketHeader::new(PacketType::AudioFrame, 12345, 2, 1_000_000),
        payload: Payload::Audio(payload),
    };

    let encoded = packet.encode().unwrap();
    let decoded = Packet::decode(encoded).unwrap();

    if let Payload::Audio(a) = decoded.payload {
        assert_eq!(a.track_id, 0);
        assert_eq!(a.sample_rate, 48000);
        assert_eq!(a.channels, 2);
        assert_eq!(a.codec, AudioCodecType::Opus);
    } else {
        panic!("Expected audio payload");
    }
}

#[test]
fn test_sync_point_encode_decode() {
    let mut sync = SyncPoint::new(1_000_000, 1_000_000);
    sync.add_track(0, 1_000_000, 0);
    sync.add_track(1, 1_001_000, 1000);

    let packet = Packet {
        header: PacketHeader::new(PacketType::SyncPoint, 12345, 3, 1_000_000),
        payload: Payload::Sync(sync),
    };

    let encoded = packet.encode().unwrap();
    let decoded = Packet::decode(encoded).unwrap();

    if let Payload::Sync(s) = decoded.payload {
        assert_eq!(s.reference_timestamp, 1_000_000);
        assert_eq!(s.video_pts, 1_000_000);
        assert_eq!(s.audio_tracks.len(), 2);
        assert_eq!(s.audio_tracks[1].offset_us, 1000);
    } else {
        panic!("Expected sync payload");
    }
}

#[test]
fn test_fec_encode_decode() {
    let config = FecConfig::new(4, 2);
    let mut encoder = FecEncoder::new(config).unwrap();
    let mut decoder = FecDecoder::new(config).unwrap();

    // Create 4 data packets
    let original_data: Vec<Bytes> = (0..4)
        .map(|i| Bytes::from(vec![i as u8; 100]))
        .collect();

    // Encode
    let group = encoder.encode(original_data.clone()).unwrap();
    assert_eq!(group.shards.len(), 6); // 4 data + 2 parity

    // Simulate loss of shards 1 and 2
    // Add shards 0, 3, 4, 5 (skipping 1 and 2)
    let indices = [0, 3, 4, 5];
    for &i in &indices[..3] {
        let result =
            decoder.add_shard(group.group_id, i, group.shards[i].clone(), group.shard_size);
        assert!(matches!(result, DecodeResult::NeedMore { .. }));
    }

    // Adding the 4th shard should trigger recovery
    let result = decoder.add_shard(
        group.group_id,
        indices[3],
        group.shards[indices[3]].clone(),
        group.shard_size,
    );

    if let DecodeResult::Recovered(recovered) = result {
        // Verify all data shards are correctly recovered
        for i in 0..4 {
            assert_eq!(
                recovered[i].as_ref(),
                original_data[i].as_ref(),
                "Shard {} mismatch",
                i
            );
        }
    } else {
        panic!("Expected Recovered result");
    }
}

#[test]
fn test_jitter_buffer() {
    let config = JitterBufferConfig {
        target_depth_us: 50_000,
        min_depth_us: 25_000,
        max_depth_us: 100_000,
        max_frames: 50,
    };

    let mut buffer = JitterBuffer::new(config);

    // Insert frames to fill buffer
    for i in 0..10 {
        let frame = ussp::protocol::VideoFrame {
            frame_id: i,
            codec: VideoCodecType::H264,
            is_key_frame: i == 0,
            pts: i as u64 * 10_000,
            dts: i as u64 * 10_000,
            data: Bytes::new(),
        };
        buffer.insert_video(frame);
    }

    // Buffer should be ready (90_000 us span >= 50_000 target)
    assert_eq!(buffer.state(), BufferState::Ready);

    // Get frames
    let frame = buffer.get_next(25_000);
    assert!(frame.is_some());

    let stats = buffer.stats();
    assert!(stats.frames_received >= 10);
}

#[test]
fn test_heartbeat_packet() {
    let packet = Packet {
        header: PacketHeader::new(PacketType::Heartbeat, 12345, 100, 5_000_000),
        payload: Payload::Heartbeat,
    };

    let encoded = packet.encode().unwrap();
    let decoded = Packet::decode(encoded).unwrap();

    assert_eq!(decoded.header.packet_type, PacketType::Heartbeat);
    assert_eq!(decoded.header.session_id, 12345);
    assert_eq!(decoded.header.sequence, 100);
    assert!(matches!(decoded.payload, Payload::Heartbeat));
}

#[test]
fn test_video_fragmentation_metadata() {
    // Test that fragmented video payloads correctly report their fragment status
    let single_fragment = VideoPayload::new(
        1,
        VideoCodecType::H264,
        true,
        1_000_000,
        900_000,
        Bytes::from(vec![0u8; 100]),
    );

    assert!(single_fragment.is_complete());
    assert!(single_fragment.is_last_fragment());

    let first_fragment = VideoPayload::new_fragment(
        1,
        0, // fragment_index
        3, // total_fragments
        VideoCodecType::H264,
        true,
        1_000_000,
        900_000,
        Bytes::from(vec![0u8; 100]),
    );

    assert!(!first_fragment.is_complete());
    assert!(!first_fragment.is_last_fragment());

    let last_fragment = VideoPayload::new_fragment(
        1,
        2, // fragment_index (0-indexed, so 2 is the last of 3)
        3, // total_fragments
        VideoCodecType::H264,
        true,
        1_000_000,
        900_000,
        Bytes::from(vec![0u8; 100]),
    );

    assert!(!last_fragment.is_complete());
    assert!(last_fragment.is_last_fragment());
}

#[test]
fn test_audio_duration_calculation() {
    let audio = AudioPayload::new(
        0,
        48000, // 48kHz
        2,
        16,
        AudioCodecType::Opus,
        0,
        48000, // 1 second worth of samples
        Bytes::new(),
    );

    // Duration should be 1 second = 1_000_000 microseconds
    assert_eq!(audio.duration_us(), 1_000_000);

    let short_audio = AudioPayload::new(
        0,
        48000,
        2,
        16,
        AudioCodecType::Opus,
        0,
        960, // 20ms worth at 48kHz
        Bytes::new(),
    );

    // Duration should be 20ms = 20_000 microseconds
    assert_eq!(short_audio.duration_us(), 20_000);
}

#[test]
fn test_dmx_packet_encode_decode() {
    // Create 512-channel DMX data
    let mut dmx_data = vec![0u8; 512];
    for i in 0..512 {
        dmx_data[i] = (i % 256) as u8;
    }

    let payload = DmxPayload::new(1, 1_000_000, Bytes::from(dmx_data.clone()));

    let packet = Packet {
        header: PacketHeader::new(PacketType::DmxFrame, 12345, 1, 1_000_000),
        payload: Payload::Dmx(payload),
    };

    let encoded = packet.encode().unwrap();
    let decoded = Packet::decode(encoded).unwrap();

    assert_eq!(decoded.header.session_id, 12345);
    assert_eq!(decoded.header.packet_type, PacketType::DmxFrame);

    if let Payload::Dmx(d) = decoded.payload {
        assert_eq!(d.universe, 1);
        assert_eq!(d.channel_count, 512);
        assert_eq!(d.pts, 1_000_000);
        assert_eq!(d.data.len(), 512);
        // Verify channel data
        for i in 0..512 {
            assert_eq!(d.data[i], (i % 256) as u8);
        }
    } else {
        panic!("Expected DMX payload");
    }
}

#[test]
fn test_dmx_artnet_addressing() {
    let payload = DmxPayload::with_artnet_address(
        1,   // net
        2,   // subnet
        3,   // universe
        42,  // sequence
        5_000_000,
        Bytes::from(vec![255u8; 512]),
    );

    // Verify Art-Net address calculation
    assert_eq!(payload.artnet_net(), 1);
    assert_eq!(payload.artnet_subnet(), 2);
    assert_eq!(payload.artnet_universe(), 3);
    assert_eq!(payload.dmx_sequence, 42);

    // Full universe: (1 << 8) | (2 << 4) | 3 = 256 + 32 + 3 = 291
    assert_eq!(payload.universe, 291);

    let packet = Packet {
        header: PacketHeader::new(PacketType::DmxFrame, 12345, 1, 5_000_000),
        payload: Payload::Dmx(payload),
    };

    let encoded = packet.encode().unwrap();
    let decoded = Packet::decode(encoded).unwrap();

    if let Payload::Dmx(d) = decoded.payload {
        assert_eq!(d.universe, 291);
        assert_eq!(d.artnet_net(), 1);
        assert_eq!(d.artnet_subnet(), 2);
        assert_eq!(d.artnet_universe(), 3);
    } else {
        panic!("Expected DMX payload");
    }
}
