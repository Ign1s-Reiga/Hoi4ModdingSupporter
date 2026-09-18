//! Turning mod and game art into something a WebView can show.
//!
//! Hearts of Iron IV ships flags as `.tga` and most interface art as `.dds`,
//! neither of which a browser can render, so those are decoded and re-encoded
//! as PNG before they reach the frontend.

use std::io::Cursor;
use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::ImageFormat;

use crate::error::{AppError, AppResult};

/// Files larger than this are always decoded and downscaled rather than
/// embedded as-is, to keep the data URL from bloating the WebView.
const PASSTHROUGH_LIMIT_BYTES: u64 = 512 * 1024;

pub fn image_data_url(path: &str, max_dimension: Option<u32>) -> AppResult<String> {
    data_url(Path::new(path), max_dimension, 1)
}

/// The first frame of a sprite.
///
/// Clausewitz stores the frames of an animated sprite side by side in one
/// texture, so a two frame goal icon is drawn from a picture twice as wide as
/// the icon: passing the whole strip through would show the icon and its
/// greyed out twin next to each other.
pub fn sprite_data_url(path: &Path, max_dimension: Option<u32>, frames: u32) -> AppResult<String> {
    data_url(path, max_dimension, frames)
}

fn data_url(file: &Path, max_dimension: Option<u32>, frames: u32) -> AppResult<String> {
    let path = file.display().to_string();
    let bytes = std::fs::read(file).map_err(|source| AppError::io(file, source))?;

    let extension = file
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let native_mime = web_native_mime(&extension);
    let needs_decode = frames > 1
        || native_mime.is_none()
        || (max_dimension.is_some() && bytes.len() as u64 > PASSTHROUGH_LIMIT_BYTES);

    if !needs_decode {
        let mime = native_mime.expect("checked above");
        return Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)));
    }

    let format = image_format(&extension).ok_or_else(|| AppError::Image {
        path: path.clone(),
        message: format!("`{extension}` files cannot be previewed"),
    })?;

    let decoded =
        image::load_from_memory_with_format(&bytes, format).map_err(|error| AppError::Image {
            path: path.clone(),
            message: error.to_string(),
        })?;

    // A frame count wider than the texture would crop it to nothing.
    let decoded = if frames > 1 && decoded.width() >= frames {
        decoded.crop_imm(0, 0, decoded.width() / frames, decoded.height())
    } else {
        decoded
    };

    let decoded = match max_dimension {
        Some(limit) if decoded.width() > limit || decoded.height() > limit => {
            decoded.thumbnail(limit, limit)
        }
        _ => decoded,
    };

    let mut encoded = Vec::new();
    decoded
        .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
        .map_err(|error| AppError::Image {
            path,
            message: error.to_string(),
        })?;

    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(&encoded)
    ))
}

/// Formats a WebView can display without any conversion.
fn web_native_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

fn image_format(extension: &str) -> Option<ImageFormat> {
    match extension {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "bmp" => Some(ImageFormat::Bmp),
        "tga" => Some(ImageFormat::Tga),
        "dds" => Some(ImageFormat::Dds),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_convertible_formats() {
        assert!(image_format("tga").is_some());
        assert!(image_format("dds").is_some());
        assert!(image_format("psd").is_none());
    }

    #[test]
    fn passes_through_web_native_formats() {
        assert_eq!(web_native_mime("png"), Some("image/png"));
        assert_eq!(web_native_mime("tga"), None);
    }

    #[test]
    fn encodes_a_generated_image_as_a_data_url() {
        let directory = std::env::temp_dir().join("hoi4ms-asset-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("swatch.png");

        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([12, 34, 56, 255]));
        image.save(&path).expect("save");

        let url = image_data_url(&path.display().to_string(), None).expect("encodes");
        assert!(url.starts_with("data:image/png;base64,"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn keeps_only_the_first_frame_of_a_sprite_strip() {
        let directory = std::env::temp_dir().join("hoi4ms-asset-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("strip.png");

        // Two frames of eight pixels, as an animated sprite stores them.
        let image = image::RgbaImage::from_pixel(16, 8, image::Rgba([12, 34, 56, 255]));
        image.save(&path).expect("save");

        let url = sprite_data_url(&path, None, 2).expect("encodes");
        let bytes = STANDARD
            .decode(url.trim_start_matches("data:image/png;base64,"))
            .expect("base64");
        let decoded = image::load_from_memory(&bytes).expect("png");

        assert_eq!((decoded.width(), decoded.height()), (8, 8));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reports_unsupported_files_clearly() {
        let directory = std::env::temp_dir().join("hoi4ms-asset-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("notes.psd");
        std::fs::write(&path, b"not an image").expect("write");

        let error = image_data_url(&path.display().to_string(), None).expect_err("fails");
        assert!(error.to_string().contains("psd"));

        std::fs::remove_file(&path).ok();
    }
}
