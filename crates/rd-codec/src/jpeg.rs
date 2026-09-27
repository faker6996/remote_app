use async_trait::async_trait;
use image::{ImageBuffer, Rgba, codecs::jpeg::JpegEncoder as ImageJpegEncoder, ImageEncoder};
use std::io::Cursor;
use tracing::{debug, warn};

use rd_core::domain::{
    models::*,
    ports::*,
    error::*,
};

/// JPEG Encoder implementation
pub struct JpegEncoder {
    config: EncoderConfig,
}

impl JpegEncoder {
    pub fn new(config: EncoderConfig) -> Self {
        Self { config }
    }
    
    pub fn with_quality(quality: u8) -> Self {
        let mut config = EncoderConfig::default();
        config.codec = CodecType::Jpeg;
        config.quality = quality.clamp(1, 100);
        Self { config }
    }
}

#[async_trait]
impl Encoder for JpegEncoder {
    async fn encode(&mut self, frame: &ScreenFrame) -> std::result::Result<Vec<u8>, CodecError> {
        debug!(
            "Encoding frame {}x{} with quality {}",
            frame.width, frame.height, self.config.quality
        );
        
        let pixel_count = (frame.width * frame.height) as usize;
        let expected_raw_len = pixel_count * 4;
        
        if frame.data.len() < expected_raw_len && frame.format != FrameFormat::Jpeg {
            return Err(CodecError::EncodingFailed(format!(
                "Frame buffer too small: expected {}, got {}",
                expected_raw_len,
                frame.data.len()
            )));
        }

        let rgb_data: Vec<u8> = match frame.format {
            FrameFormat::Bgra => {
                // macOS ScreenCaptureKit: BGRA -> RGB
                let mut rgb = Vec::with_capacity(pixel_count * 3);
                for chunk in frame.data[..expected_raw_len].chunks_exact(4) {
                    rgb.push(chunk[2]); // R
                    rgb.push(chunk[1]); // G
                    rgb.push(chunk[0]); // B
                }
                rgb
            }
            FrameFormat::Rgba | FrameFormat::Raw => {
                // RGBA -> RGB
                let mut rgb = Vec::with_capacity(pixel_count * 3);
                for chunk in frame.data[..expected_raw_len].chunks_exact(4) {
                    rgb.push(chunk[0]); // R
                    rgb.push(chunk[1]); // G
                    rgb.push(chunk[2]); // B
                }
                rgb
            }
            _ => {
                return Err(CodecError::EncodingFailed(
                    format!("Unsupported input format: {:?}", frame.format)
                ));
            }
        };
        
        // Encode to JPEG
        let mut buffer = Cursor::new(Vec::new());
        let encoder = ImageJpegEncoder::new_with_quality(&mut buffer, self.config.quality);
        
        encoder
            .write_image(
                &rgb_data,
                frame.width,
                frame.height,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| CodecError::EncodingFailed(e.to_string()))?;
        
        let encoded = buffer.into_inner();
        
        debug!("Encoded {} bytes (compression ratio: {:.2}x)", 
            encoded.len(),
            frame.data.len() as f32 / encoded.len() as f32
        );
        
        Ok(encoded)
    }
    
    fn config(&self) -> &EncoderConfig {
        &self.config
    }
    
    fn set_config(&mut self, config: EncoderConfig) -> std::result::Result<(), CodecError> {
        if config.codec != CodecType::Jpeg {
            return Err(CodecError::InvalidConfig(
                "JpegEncoder only supports JPEG codec".to_string()
            ));
        }
        self.config = config;
        Ok(())
    }
}

/// JPEG Decoder implementation
pub struct JpegDecoder;

impl JpegDecoder {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JpegDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Decoder for JpegDecoder {
    async fn decode(&mut self, data: &[u8]) -> std::result::Result<ScreenFrame, CodecError> {
        debug!("Decoding JPEG data ({} bytes)", data.len());
        
        let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg)
            .map_err(|e| CodecError::DecodingFailed(e.to_string()))?;
        
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();
        
        let frame = ScreenFrame {
            sequence: 0, // Will be set by caller
            timestamp: 0, // Will be set by caller
            data: rgba_img.into_raw(),
            width,
            height,
            format: FrameFormat::Rgba,
        };
        
        debug!("Decoded frame {}x{}", width, height);
        
        Ok(frame)
    }
    
    fn codec_type(&self) -> CodecType {
        CodecType::Jpeg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_jpeg_encode_decode() {
        // Create a test frame (simple gradient)
        let width = 640;
        let height = 480;
        let mut data = Vec::with_capacity((width * height * 4) as usize);
        
        for y in 0..height {
            for x in 0..width {
                data.push((x % 256) as u8); // R
                data.push((y % 256) as u8); // G
                data.push(128); // B
                data.push(255); // A
            }
        }
        
        let frame = ScreenFrame {
            sequence: 0,
            timestamp: 0,
            data,
            width,
            height,
            format: FrameFormat::Raw,
        };
        
        // Encode
        let mut encoder = JpegEncoder::with_quality(80);
        let encoded = encoder.encode(&frame).await.unwrap();
        assert!(!encoded.is_empty());
        
        // Decode
        let mut decoder = JpegDecoder::new();
        let decoded = decoder.decode(&encoded).await.unwrap();
        assert_eq!(decoded.width, width);
        assert_eq!(decoded.height, height);
    }
}
