use async_trait::async_trait;
use openh264::encoder::Encoder as NativeEncoder;
use openh264::decoder::Decoder as NativeDecoder;
use openh264::formats::YUVSource;
use std::sync::Mutex;
use tracing::debug;

use rd_core::domain::{
    error::CodecError,
    models::{CodecType, EncoderConfig, FrameFormat, ScreenFrame},
    ports::{Decoder, Encoder},
};
use crate::color_convert::bgra_to_yuv_buffer;

/// H.264 Video Encoder implementation using OpenH264
pub struct H264Encoder {
    encoder: Mutex<NativeEncoder>,
    config: EncoderConfig,
    frame_count: u64,
    gop_size: u64, // Keyframe interval (default: 60 frames = 2 seconds at 30 FPS)
}

impl H264Encoder {
    pub fn new(config: EncoderConfig) -> Result<Self, CodecError> {
        let encoder = NativeEncoder::new()
            .map_err(|e| CodecError::Other(e.to_string()))?;
        
        Ok(Self {
            encoder: Mutex::new(encoder),
            config,
            frame_count: 0,
            gop_size: 60,
        })
    }

    pub fn with_bitrate(bitrate_kbps: u32) -> Result<Self, CodecError> {
        let mut config = EncoderConfig::default();
        config.codec = CodecType::H264;
        config.bitrate = Some(bitrate_kbps);
        config.target_fps = 30;
        Self::new(config)
    }

    /// Set Keyframe interval (GOP size in frames)
    pub fn set_gop_size(&mut self, gop: u64) {
        self.gop_size = gop.max(1);
    }
}

#[async_trait]
impl Encoder for H264Encoder {
    async fn encode(&mut self, frame: &ScreenFrame) -> Result<Vec<u8>, CodecError> {
        let width = frame.width as usize;
        let height = frame.height as usize;

        if width == 0 || height == 0 || frame.data.is_empty() {
            return Err(CodecError::EncodingFailed("Empty frame data".into()));
        }

        // Convert BGRA to YUV420p
        let yuv_buf = bgra_to_yuv_buffer(width, height, &frame.data);

        let mut encoder = self.encoder.lock().map_err(|_| {
            CodecError::EncodingFailed("Failed to acquire encoder lock".into())
        })?;

        // Periodically force an Intra-frame (IDR / Keyframe)
        if self.frame_count % self.gop_size == 0 {
            encoder.force_intra_frame();
        }
        self.frame_count = self.frame_count.wrapping_add(1);

        // Encode frame to H.264 bitstream
        let bitstream = encoder.encode(&yuv_buf)
            .map_err(|e| CodecError::EncodingFailed(format!("OpenH264 encode error: {}", e)))?;

        let bytes = bitstream.to_vec();
        debug!(
            "H.264 encoded frame {}: {} bytes (orig: {}x{})",
            self.frame_count,
            bytes.len(),
            width,
            height
        );
        Ok(bytes)
    }

    fn config(&self) -> &EncoderConfig {
        &self.config
    }

    fn set_config(&mut self, config: EncoderConfig) -> Result<(), CodecError> {
        self.config = config;
        Ok(())
    }
}

/// H.264 Video Decoder implementation using OpenH264
pub struct H264Decoder {
    decoder: Mutex<NativeDecoder>,
}

impl H264Decoder {
    pub fn new() -> Result<Self, CodecError> {
        let decoder = NativeDecoder::new()
            .map_err(|e| CodecError::Other(e.to_string()))?;
        Ok(Self {
            decoder: Mutex::new(decoder),
        })
    }
}

#[async_trait]
impl Decoder for H264Decoder {
    async fn decode(&mut self, data: &[u8]) -> Result<ScreenFrame, CodecError> {
        let mut decoder = self.decoder.lock().map_err(|_| {
            CodecError::DecodingFailed("Failed to acquire decoder lock".into())
        })?;

        // Decode NAL packet
        let decoded = decoder.decode(data)
            .map_err(|e| CodecError::DecodingFailed(format!("OpenH264 decode error: {}", e)))?;

        if let Some(yuv) = decoded {
            let (w, h) = yuv.dimensions();
            let mut rgba = vec![0u8; w * h * 4];
            yuv.write_rgba8(&mut rgba);

            Ok(ScreenFrame {
                sequence: 0,
                timestamp: 0,
                data: rgba,
                width: w as u32,
                height: h as u32,
                format: FrameFormat::Rgba,
            })
        } else {
            Err(CodecError::DecodingFailed("No frame produced from NAL unit".into()))
        }
    }

    fn codec_type(&self) -> CodecType {
        CodecType::H264
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_h264_encode_decode_roundtrip() {
        let width = 64u32;
        let height = 64u32;
        // Generate a 64x64 test pattern in BGRA
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        for (i, pixel) in bgra.chunks_exact_mut(4).enumerate() {
            pixel[0] = (i % 255) as u8;       // B
            pixel[1] = ((i * 2) % 255) as u8; // G
            pixel[2] = 200;                  // R
            pixel[3] = 255;                  // A
        }

        let frame = ScreenFrame {
            sequence: 1,
            timestamp: 1000,
            data: bgra,
            width,
            height,
            format: FrameFormat::Bgra,
        };

        let mut encoder = H264Encoder::with_bitrate(2000).expect("Create encoder");
        let encoded_bytes = encoder.encode(&frame).await.expect("Encode frame");
        assert!(!encoded_bytes.is_empty(), "Encoded H.264 stream should not be empty");

        let mut decoder = H264Decoder::new().expect("Create decoder");
        let decoded_frame = decoder.decode(&encoded_bytes).await.expect("Decode frame");
        assert_eq!(decoded_frame.width, width);
        assert_eq!(decoded_frame.height, height);
        assert_eq!(decoded_frame.data.len(), (width * height * 4) as usize);
    }
}
