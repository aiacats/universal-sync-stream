//! Motion capture frame payload definitions (NatNet compatible).

use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// MocapPayload fixed header size in bytes.
pub const MOCAP_PAYLOAD_HEADER_SIZE: usize = 36;

/// Size of a single rigid body record in the wire format.
pub const RIGID_BODY_SIZE: usize = 38;

/// Size of a skeleton header in the wire format.
pub const SKELETON_HEADER_SIZE: usize = 8;

/// Size of a single labeled marker record in the wire format.
pub const LABELED_MARKER_SIZE: usize = 26;

/// A rigid body tracked by the motion capture system (6DOF).
#[derive(Debug, Clone)]
pub struct RigidBody {
    /// Streaming ID assigned in the motion capture software.
    pub id: i32,
    /// Position in meters [x, y, z].
    pub position: [f32; 3],
    /// Orientation quaternion [qx, qy, qz, qw].
    pub rotation: [f32; 4],
    /// Mean measure-to-solve deviation in meters.
    pub mean_error: f32,
    /// Whether tracking is currently valid.
    pub tracking_valid: bool,
}

/// A skeleton consisting of hierarchically connected bone rigid bodies.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// Skeleton ID assigned in the motion capture software.
    pub id: i32,
    /// Bone rigid bodies comprising this skeleton.
    pub bones: Vec<RigidBody>,
}

/// A labeled marker tracked by the motion capture system.
#[derive(Debug, Clone)]
pub struct LabeledMarker {
    /// Composite marker ID (encodes model/marker indices).
    pub id: i32,
    /// Position in meters [x, y, z].
    pub position: [f32; 3],
    /// Estimated marker diameter in meters.
    pub size: f32,
    /// Tracking flags (occluded, point_cloud_solved, model_solved, etc.).
    pub params: u16,
    /// Tracking error residual in meters.
    pub residual: f32,
}

/// Motion capture frame payload.
///
/// Carries NatNet-compatible motion capture data synchronized with video/audio.
#[derive(Debug, Clone)]
pub struct MocapPayload {
    /// Presentation timestamp in microseconds (USSP synchronization).
    pub pts: u64,
    /// NatNet frame number.
    pub frame_number: u32,
    /// SMPTE timecode.
    pub timecode: u32,
    /// SMPTE timecode sub-frame.
    pub timecode_subframe: u32,
    /// NatNet timestamp (seconds since software start).
    pub timestamp: f64,
    /// Frame-level flags.
    pub params: u16,
    /// Tracked rigid bodies.
    pub rigid_bodies: Vec<RigidBody>,
    /// Tracked skeletons.
    pub skeletons: Vec<Skeleton>,
    /// Tracked labeled markers.
    pub labeled_markers: Vec<LabeledMarker>,
}

impl MocapPayload {
    /// Create a new motion capture payload.
    pub fn new(pts: u64, frame_number: u32) -> Self {
        Self {
            pts,
            frame_number,
            timecode: 0,
            timecode_subframe: 0,
            timestamp: 0.0,
            params: 0,
            rigid_bodies: Vec::new(),
            skeletons: Vec::new(),
            labeled_markers: Vec::new(),
        }
    }

    /// Encode the motion capture payload to bytes.
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        // Header (36 bytes)
        buf.put_u64(self.pts);
        buf.put_u32(self.frame_number);
        buf.put_u32(self.timecode);
        buf.put_u32(self.timecode_subframe);
        buf.put_u64(self.timestamp.to_bits());
        buf.put_u16(self.rigid_bodies.len() as u16);
        buf.put_u16(self.skeletons.len() as u16);
        buf.put_u16(self.labeled_markers.len() as u16);
        buf.put_u16(self.params);

        // Rigid bodies
        for rb in &self.rigid_bodies {
            encode_rigid_body(rb, buf);
        }

        // Skeletons
        for sk in &self.skeletons {
            buf.put_i32(sk.id);
            buf.put_u32(sk.bones.len() as u32);
            for bone in &sk.bones {
                encode_rigid_body(bone, buf);
            }
        }

        // Labeled markers
        for lm in &self.labeled_markers {
            buf.put_i32(lm.id);
            buf.put_f32(lm.position[0]);
            buf.put_f32(lm.position[1]);
            buf.put_f32(lm.position[2]);
            buf.put_f32(lm.size);
            buf.put_u16(lm.params);
            buf.put_f32(lm.residual);
        }

        Ok(())
    }

    /// Decode a motion capture payload from bytes.
    pub fn decode(buf: &mut Bytes) -> Result<Self> {
        if buf.remaining() < MOCAP_PAYLOAD_HEADER_SIZE {
            return Err(Error::BufferTooSmall {
                needed: MOCAP_PAYLOAD_HEADER_SIZE,
                got: buf.remaining(),
            });
        }

        let pts = buf.get_u64();
        let frame_number = buf.get_u32();
        let timecode = buf.get_u32();
        let timecode_subframe = buf.get_u32();
        let timestamp = f64::from_bits(buf.get_u64());
        let rigid_body_count = buf.get_u16() as usize;
        let skeleton_count = buf.get_u16() as usize;
        let labeled_marker_count = buf.get_u16() as usize;
        let params = buf.get_u16();

        // Decode rigid bodies
        if buf.remaining() < rigid_body_count * RIGID_BODY_SIZE {
            return Err(Error::BufferTooSmall {
                needed: rigid_body_count * RIGID_BODY_SIZE,
                got: buf.remaining(),
            });
        }
        let mut rigid_bodies = Vec::with_capacity(rigid_body_count);
        for _ in 0..rigid_body_count {
            rigid_bodies.push(decode_rigid_body(buf)?);
        }

        // Decode skeletons
        let mut skeletons = Vec::with_capacity(skeleton_count);
        for _ in 0..skeleton_count {
            if buf.remaining() < SKELETON_HEADER_SIZE {
                return Err(Error::BufferTooSmall {
                    needed: SKELETON_HEADER_SIZE,
                    got: buf.remaining(),
                });
            }
            let id = buf.get_i32();
            let bone_count = buf.get_u32() as usize;
            if buf.remaining() < bone_count * RIGID_BODY_SIZE {
                return Err(Error::BufferTooSmall {
                    needed: bone_count * RIGID_BODY_SIZE,
                    got: buf.remaining(),
                });
            }
            let mut bones = Vec::with_capacity(bone_count);
            for _ in 0..bone_count {
                bones.push(decode_rigid_body(buf)?);
            }
            skeletons.push(Skeleton { id, bones });
        }

        // Decode labeled markers
        if buf.remaining() < labeled_marker_count * LABELED_MARKER_SIZE {
            return Err(Error::BufferTooSmall {
                needed: labeled_marker_count * LABELED_MARKER_SIZE,
                got: buf.remaining(),
            });
        }
        let mut labeled_markers = Vec::with_capacity(labeled_marker_count);
        for _ in 0..labeled_marker_count {
            labeled_markers.push(decode_labeled_marker(buf)?);
        }

        Ok(Self {
            pts,
            frame_number,
            timecode,
            timecode_subframe,
            timestamp,
            params,
            rigid_bodies,
            skeletons,
            labeled_markers,
        })
    }
}

/// Encode a rigid body to the buffer.
fn encode_rigid_body(rb: &RigidBody, buf: &mut BytesMut) {
    buf.put_i32(rb.id);
    buf.put_f32(rb.position[0]);
    buf.put_f32(rb.position[1]);
    buf.put_f32(rb.position[2]);
    buf.put_f32(rb.rotation[0]);
    buf.put_f32(rb.rotation[1]);
    buf.put_f32(rb.rotation[2]);
    buf.put_f32(rb.rotation[3]);
    buf.put_f32(rb.mean_error);
    buf.put_u16(if rb.tracking_valid { 0x0001 } else { 0x0000 });
}

/// Decode a rigid body from the buffer.
fn decode_rigid_body(buf: &mut Bytes) -> Result<RigidBody> {
    let id = buf.get_i32();
    let position = [buf.get_f32(), buf.get_f32(), buf.get_f32()];
    let rotation = [
        buf.get_f32(),
        buf.get_f32(),
        buf.get_f32(),
        buf.get_f32(),
    ];
    let mean_error = buf.get_f32();
    let params = buf.get_u16();

    Ok(RigidBody {
        id,
        position,
        rotation,
        mean_error,
        tracking_valid: (params & 0x0001) != 0,
    })
}

/// Decode a labeled marker from the buffer.
fn decode_labeled_marker(buf: &mut Bytes) -> Result<LabeledMarker> {
    let id = buf.get_i32();
    let position = [buf.get_f32(), buf.get_f32(), buf.get_f32()];
    let size = buf.get_f32();
    let params = buf.get_u16();
    let residual = buf.get_f32();

    Ok(LabeledMarker {
        id,
        position,
        size,
        params,
        residual,
    })
}

/// A complete motion capture frame.
#[derive(Debug, Clone)]
pub struct MocapFrame {
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// NatNet frame number.
    pub frame_number: u32,
    /// SMPTE timecode.
    pub timecode: u32,
    /// SMPTE timecode sub-frame.
    pub timecode_subframe: u32,
    /// NatNet timestamp (seconds since software start).
    pub timestamp: f64,
    /// Frame-level flags.
    pub params: u16,
    /// Tracked rigid bodies.
    pub rigid_bodies: Vec<RigidBody>,
    /// Tracked skeletons.
    pub skeletons: Vec<Skeleton>,
    /// Tracked labeled markers.
    pub labeled_markers: Vec<LabeledMarker>,
}

impl From<MocapPayload> for MocapFrame {
    fn from(payload: MocapPayload) -> Self {
        Self {
            pts: payload.pts,
            frame_number: payload.frame_number,
            timecode: payload.timecode,
            timecode_subframe: payload.timecode_subframe,
            timestamp: payload.timestamp,
            params: payload.params,
            rigid_bodies: payload.rigid_bodies,
            skeletons: payload.skeletons,
            labeled_markers: payload.labeled_markers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mocap_payload_roundtrip() {
        let mut payload = MocapPayload::new(1_000_000, 42);
        payload.timecode = 0x01020304;
        payload.timecode_subframe = 0x00000005;
        payload.timestamp = 1.234567;
        payload.params = 0x0001;

        payload.rigid_bodies.push(RigidBody {
            id: 1,
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            mean_error: 0.001,
            tracking_valid: true,
        });

        payload.skeletons.push(Skeleton {
            id: 1,
            bones: vec![
                RigidBody {
                    id: 100,
                    position: [0.5, 1.5, 2.5],
                    rotation: [0.1, 0.2, 0.3, 0.9],
                    mean_error: 0.002,
                    tracking_valid: true,
                },
                RigidBody {
                    id: 101,
                    position: [0.6, 1.6, 2.6],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    mean_error: 0.003,
                    tracking_valid: false,
                },
            ],
        });

        payload.labeled_markers.push(LabeledMarker {
            id: 10,
            position: [0.1, 0.2, 0.3],
            size: 0.014,
            params: 0x0003,
            residual: 0.0005,
        });

        let mut buf = BytesMut::new();
        payload.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = MocapPayload::decode(&mut bytes).unwrap();

        assert_eq!(decoded.pts, 1_000_000);
        assert_eq!(decoded.frame_number, 42);
        assert_eq!(decoded.timecode, 0x01020304);
        assert_eq!(decoded.timecode_subframe, 0x00000005);
        assert!((decoded.timestamp - 1.234567).abs() < f64::EPSILON);
        assert_eq!(decoded.params, 0x0001);

        // Rigid bodies
        assert_eq!(decoded.rigid_bodies.len(), 1);
        assert_eq!(decoded.rigid_bodies[0].id, 1);
        assert_eq!(decoded.rigid_bodies[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(decoded.rigid_bodies[0].rotation, [0.0, 0.0, 0.0, 1.0]);
        assert!((decoded.rigid_bodies[0].mean_error - 0.001).abs() < f32::EPSILON);
        assert!(decoded.rigid_bodies[0].tracking_valid);

        // Skeletons
        assert_eq!(decoded.skeletons.len(), 1);
        assert_eq!(decoded.skeletons[0].id, 1);
        assert_eq!(decoded.skeletons[0].bones.len(), 2);
        assert_eq!(decoded.skeletons[0].bones[0].id, 100);
        assert!(decoded.skeletons[0].bones[0].tracking_valid);
        assert_eq!(decoded.skeletons[0].bones[1].id, 101);
        assert!(!decoded.skeletons[0].bones[1].tracking_valid);

        // Labeled markers
        assert_eq!(decoded.labeled_markers.len(), 1);
        assert_eq!(decoded.labeled_markers[0].id, 10);
        assert_eq!(decoded.labeled_markers[0].position, [0.1, 0.2, 0.3]);
        assert!((decoded.labeled_markers[0].size - 0.014).abs() < f32::EPSILON);
        assert_eq!(decoded.labeled_markers[0].params, 0x0003);
    }

    #[test]
    fn test_empty_mocap_payload_roundtrip() {
        let payload = MocapPayload::new(0, 0);
        let mut buf = BytesMut::new();
        payload.encode(&mut buf).unwrap();
        let mut bytes = buf.freeze();
        let decoded = MocapPayload::decode(&mut bytes).unwrap();

        assert_eq!(decoded.pts, 0);
        assert_eq!(decoded.frame_number, 0);
        assert_eq!(decoded.rigid_bodies.len(), 0);
        assert_eq!(decoded.skeletons.len(), 0);
        assert_eq!(decoded.labeled_markers.len(), 0);
    }

    #[test]
    fn test_rigid_body_tracking_flags() {
        let rb_valid = RigidBody {
            id: 1,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            mean_error: 0.0,
            tracking_valid: true,
        };
        let rb_invalid = RigidBody {
            id: 2,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            mean_error: 0.0,
            tracking_valid: false,
        };

        let mut buf = BytesMut::new();
        encode_rigid_body(&rb_valid, &mut buf);
        encode_rigid_body(&rb_invalid, &mut buf);

        let mut bytes = buf.freeze();
        let decoded_valid = decode_rigid_body(&mut bytes).unwrap();
        let decoded_invalid = decode_rigid_body(&mut bytes).unwrap();

        assert!(decoded_valid.tracking_valid);
        assert!(!decoded_invalid.tracking_valid);
    }

    #[test]
    fn test_buffer_too_small_header() {
        let mut bytes = Bytes::from(vec![0u8; MOCAP_PAYLOAD_HEADER_SIZE - 1]);
        let result = MocapPayload::decode(&mut bytes);
        assert!(matches!(result, Err(Error::BufferTooSmall { .. })));
    }

    #[test]
    fn test_large_mocap_frame() {
        let mut payload = MocapPayload::new(5_000_000, 120);
        payload.timestamp = 42.5;

        // 10 rigid bodies
        for i in 0..10 {
            payload.rigid_bodies.push(RigidBody {
                id: i,
                position: [i as f32 * 0.1, i as f32 * 0.2, i as f32 * 0.3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                mean_error: 0.001,
                tracking_valid: true,
            });
        }

        // 1 skeleton with 24 bones
        let bones: Vec<RigidBody> = (0..24)
            .map(|i| RigidBody {
                id: 1000 + i,
                position: [i as f32 * 0.01, i as f32 * 0.02, i as f32 * 0.03],
                rotation: [0.0, 0.0, 0.0, 1.0],
                mean_error: 0.002,
                tracking_valid: true,
            })
            .collect();
        payload.skeletons.push(Skeleton { id: 1, bones });

        // 50 labeled markers
        for i in 0..50 {
            payload.labeled_markers.push(LabeledMarker {
                id: 2000 + i,
                position: [i as f32 * 0.05, i as f32 * 0.06, i as f32 * 0.07],
                size: 0.014,
                params: 0x0001,
                residual: 0.0003,
            });
        }

        let mut buf = BytesMut::new();
        payload.encode(&mut buf).unwrap();
        let encoded_len = buf.len();
        let mut bytes = buf.freeze();
        let decoded = MocapPayload::decode(&mut bytes).unwrap();

        assert_eq!(decoded.rigid_bodies.len(), 10);
        assert_eq!(decoded.skeletons.len(), 1);
        assert_eq!(decoded.skeletons[0].bones.len(), 24);
        assert_eq!(decoded.labeled_markers.len(), 50);

        // Verify expected payload size: 36 + (10*38) + (8 + 24*38) + (50*26) = 2636
        assert_eq!(
            encoded_len,
            MOCAP_PAYLOAD_HEADER_SIZE
                + 10 * RIGID_BODY_SIZE
                + SKELETON_HEADER_SIZE
                + 24 * RIGID_BODY_SIZE
                + 50 * LABELED_MARKER_SIZE
        );
    }
}
