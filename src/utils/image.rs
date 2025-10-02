use std::borrow::Cow;
use std::io::Cursor;

use axum::body::Bytes;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader};
use jpeg_encoder::{ColorType, Encoder};

use crate::prelude::*;

pub async fn decode(bytes: &Bytes) -> AppResult<DynamicImage> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AppError::BadRequest)?
        .decode()
        .map_err(|_| AppError::BadRequest)
}

pub async fn encode_jpeg(image: &DynamicImage, max_w: Option<u32>) -> Vec<u8> {
    // Arrived experimentally by testing a variety of event flyers.
    // This seems like the optimal size/quality tradeoff, with a slight pref for quality.
    const QUALITY: u8 = 85;

    let mut image = Cow::Borrowed(image);
    if let Some(max_width) = max_w
        && image.width() > max_width
    {
        let ratio = max_width as f32 / image.width() as f32;
        let new_height = (image.height() as f32 * ratio) as u32;
        image = Cow::Owned(image.resize(max_width, new_height, FilterType::Lanczos3));
    }

    let rgb = image.to_rgb8();
    let mut bytes = vec![];
    let mut encoder = Encoder::new(&mut bytes, QUALITY);

    // Slightly smaller file sizes in exchange for slightly slower encoding performance (<100ms)
    encoder.set_optimized_huffman_tables(true);
    // Allows slow clients to stream in a low-res version first, and add higher quality in passes.
    // For a 2.1MB image with simulated 1.44Mb/s download speed, we see an initial paint at ~1.4s
    // and full detail at ~12.3s. We'd otherwise have to wait the full 12s to see the complete image.
    // Note: 10 is the default used by libjpegturbo.
    encoder.set_progressive(true);
    encoder.set_progressive_scans(10);

    encoder
        .encode(rgb.as_raw(), rgb.width() as u16, rgb.height() as u16, ColorType::Rgb)
        .expect("JPEG encoding should not fail");
    bytes
}
