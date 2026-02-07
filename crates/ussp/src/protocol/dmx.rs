//! DMX frame payload definitions (Art-Net compatible).

use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// DMX payload header size.
pub const DMX_PAYLOAD_HEADER_SIZE: usize = 16;

/// Maximum DMX channels per universe.
pub const DMX_CHANNELS_PER_UNIVERSE: usize = 512;

/// DMX frame payload.
///
/// Carries DMX512 data for a single universe, synchronized with video/audio.
#[derive(Debug, Clone)]
pub struct DmxPayload {
    /// Universe number (0-32767, Art-Net compatible).
    pub universe: u16,
    /// Net (upper 7 bits of 15-bit port address, Art-Net style).
    pub net: u8,
    /// Sub-Net (bits 4-7 of lower 8 bits, Art-Net style).
    pub subnet: u8,
    /// DMX sequence number (0-255, wraps around).
    pub dmx_sequence: u8,
    /// Number of channels in this frame (1-512).
    pub channel_count: u16,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// DMX channel data (1-512 bytes).
    pub data: Bytes,
}

impl DmxPayload {
    /// Create a new DMX payload.
    pub fn new(universe: u16, pts: u64, data: Bytes) -> Self {
        Self {
            universe,
            net: 0,
            subnet: 0,
            dmx_sequence: 0,
            channel_count: data.len().min(DMX_CHANNELS_PER_UNIVERSE) as u16,
            pts,
            data,
        }
    }

    /// Create a new DMX payload with Art-Net addressing.
    pub fn with_artnet_address(
        net: u8,
        subnet: u8,
        universe: u8,
        dmx_sequence: u8,
        pts: u64,
        data: Bytes,
    ) -> Self {
        // Art-Net port address: Net (7 bits) | Sub-Net (4 bits) | Universe (4 bits)
        let full_universe = ((net as u16 & 0x7F) << 8)
            | ((subnet as u16 & 0x0F) << 4)
            | (universe as u16 & 0x0F);

        Self {
            universe: full_universe,
            net,
            subnet,
            dmx_sequence,
            channel_count: data.len().min(DMX_CHANNELS_PER_UNIVERSE) as u16,
            pts,
            data,
        }
    }

    /// Get the Art-Net universe (lower 4 bits).
    pub fn artnet_universe(&self) -> u8 {
        (self.universe & 0x0F) as u8
    }

    /// Get the Art-Net sub-net (bits 4-7).
    pub fn artnet_subnet(&self) -> u8 {
        ((self.universe >> 4) & 0x0F) as u8
    }

    /// Get the Art-Net net (bits 8-14).
    pub fn artnet_net(&self) -> u8 {
        ((self.universe >> 8) & 0x7F) as u8
    }

    /// Encode the DMX payload to bytes.
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_u16(self.universe);
        buf.put_u8(self.net);
        buf.put_u8(self.subnet);
        buf.put_u8(self.dmx_sequence);
        buf.put_u8(0); // reserved
        buf.put_u16(self.channel_count);
        buf.put_u64(self.pts);
        buf.put_slice(&self.data[..self.channel_count as usize]);
        Ok(())
    }

    /// Decode a DMX payload from bytes.
    pub fn decode(buf: &mut Bytes) -> Result<Self> {
        if buf.remaining() < DMX_PAYLOAD_HEADER_SIZE {
            return Err(Error::BufferTooSmall {
                needed: DMX_PAYLOAD_HEADER_SIZE,
                got: buf.remaining(),
            });
        }

        let universe = buf.get_u16();
        let net = buf.get_u8();
        let subnet = buf.get_u8();
        let dmx_sequence = buf.get_u8();
        let _reserved = buf.get_u8();
        let channel_count = buf.get_u16();
        let pts = buf.get_u64();

        let data_len = (channel_count as usize).min(DMX_CHANNELS_PER_UNIVERSE);
        if buf.remaining() < data_len {
            return Err(Error::BufferTooSmall {
                needed: data_len,
                got: buf.remaining(),
            });
        }

        let data = buf.copy_to_bytes(data_len);

        Ok(Self {
            universe,
            net,
            subnet,
            dmx_sequence,
            channel_count,
            pts,
            data,
        })
    }
}

/// A complete DMX frame.
#[derive(Debug, Clone)]
pub struct DmxFrame {
    /// Universe number (0-32767).
    pub universe: u16,
    /// Net (Art-Net).
    pub net: u8,
    /// Sub-Net (Art-Net).
    pub subnet: u8,
    /// DMX sequence number.
    pub dmx_sequence: u8,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// DMX channel data.
    pub data: Bytes,
}

impl From<DmxPayload> for DmxFrame {
    fn from(payload: DmxPayload) -> Self {
        Self {
            universe: payload.universe,
            net: payload.net,
            subnet: payload.subnet,
            dmx_sequence: payload.dmx_sequence,
            pts: payload.pts,
            data: payload.data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dmx_payload_roundtrip() {
        let mut dmx_data = vec![0u8; 512];
        for i in 0..512 {
            dmx_data[i] = (i % 256) as u8;
        }

        let payload = DmxPayload::new(1, 1_000_000, Bytes::from(dmx_data.clone()));

        let mut buf = BytesMut::new();
        payload.encode(&mut buf).unwrap();

        let mut bytes = buf.freeze();
        let decoded = DmxPayload::decode(&mut bytes).unwrap();

        assert_eq!(decoded.universe, 1);
        assert_eq!(decoded.channel_count, 512);
        assert_eq!(decoded.pts, 1_000_000);
        assert_eq!(decoded.data.len(), 512);
    }

    #[test]
    fn test_artnet_addressing() {
        let payload = DmxPayload::with_artnet_address(
            1,   // net
            2,   // subnet
            3,   // universe
            0,   // sequence
            0,   // pts
            Bytes::from(vec![0u8; 512]),
        );

        assert_eq!(payload.artnet_net(), 1);
        assert_eq!(payload.artnet_subnet(), 2);
        assert_eq!(payload.artnet_universe(), 3);

        // Full universe: (1 << 8) | (2 << 4) | 3 = 256 + 32 + 3 = 291
        assert_eq!(payload.universe, 291);
    }
}
