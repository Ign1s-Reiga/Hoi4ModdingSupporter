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
    let file = Path::new(path);
    let bytes = std::fs::read(file).map_err(|source| AppError::io(file, source))?;

    let extension = file
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let native_mime = web_native_mime(&extension);
    let needs_decode = native_mime.is_none()
        || (max_dimension.is_some() && bytes.len() as u64 > PASSTHROUGH_LIMIT_BYTES);

    if !needs_decode {
        let mime = native_mime.expect("checked above");
        return Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)));
    }

    let format = image_format(&extension).ok_or_else(|| AppError::Image {
        path: path.to_string(),
        message: format!("`{extension}` files cannot be previewed"),
    })?;

    let decoded =
        image::load_from_memory_with_format(&bytes, format).map_err(|error| AppError::Image {
            path: path.to_string(),
            message: error.to_string(),
        })?;

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
            path: path.to_string(),
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
