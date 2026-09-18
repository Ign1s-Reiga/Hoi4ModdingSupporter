//! Turning mod and game art into something a WebView can show.
//!
//! Hearts of Iron IV ships flags as `.tga` and most interface art as `.dds`,
//! neither of which a browser can render, so those are decoded and re-encoded
//! as PNG before they reach the frontend.

use std::io::Cursor;
use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::{DynamicImage, ImageFormat};
use image_dds::ddsfile::Dds;

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

    let format = detect_format(&bytes, &extension).ok_or_else(|| AppError::Image {
        path: path.clone(),
        message: format!("`{extension}` files cannot be previewed"),
    })?;

    let native_mime = web_native_mime(format);
    let needs_decode = frames > 1
        || native_mime.is_none()
        || (max_dimension.is_some() && bytes.len() as u64 > PASSTHROUGH_LIMIT_BYTES);

    if !needs_decode {
        let mime = native_mime.expect("checked above");
        return Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)));
    }

    let decoded = decode(&bytes, format).map_err(|message| AppError::Image {
        path: path.clone(),
        message,
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

/// What the bytes are, falling back to what the file is called.
///
/// The header is asked first because mods ship art whose name says one thing
/// and whose content says another — a PNG saved as `.dds` is common, and the
/// game loads it, because it reads the header too. `.tga` has no magic number
/// of its own, which is why the extension still has the last word.
fn detect_format(bytes: &[u8], extension: &str) -> Option<ImageFormat> {
    image::guess_format(bytes)
        .ok()
        .or_else(|| image_format(extension))
}

/// Decodes a texture, DDS through `image_dds` and everything else through
/// `image`.
///
/// `image` reads only the DXT compressed DDS variants, and nearly every
/// texture the game ships is uncompressed — a masked `A8R8G8B8` surface — so
/// its own decoder would refuse the whole `gfx` folder. `image_dds` reads
/// those, the DXT ones, and the BC4 to BC7 compressions a mod may have saved.
///
/// The error is a message because that is all it is ever used for: the caller
/// pairs it with the path that failed.
fn decode(bytes: &[u8], format: ImageFormat) -> Result<DynamicImage, String> {
    if format == ImageFormat::Dds {
        let dds = Dds::read(bytes).map_err(|error| error.to_string())?;
        // Mip level zero is the full sized surface.
        let surface = image_dds::image_from_dds(&dds, 0).map_err(|error| error.to_string())?;

        return Ok(DynamicImage::ImageRgba8(surface));
    }

    image::load_from_memory_with_format(bytes, format).map_err(|error| error.to_string())
}

/// Formats a WebView can display without any conversion.
fn web_native_mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Gif => Some("image/gif"),
        ImageFormat::Bmp => Some("image/bmp"),
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
        assert_eq!(web_native_mime(ImageFormat::Png), Some("image/png"));
        assert_eq!(web_native_mime(ImageFormat::Tga), None);
    }

    /// A one pixel uncompressed `B8G8R8A8` surface, the layout the game saves
    /// its interface art in.
    fn uncompressed_dds() -> Vec<u8> {
        let mut bytes = Vec::from(*b"DDS ");

        let mut header = [0u32; 31];
        header[0] = 124;
        header[1] = 0x1_00f;
        header[2] = 1;
        header[3] = 1;
        header[4] = 4;
        header[18] = 32;
        header[19] = 0x41;
        header[21] = 32;
        header[22] = 0x00ff_0000;
        header[23] = 0x0000_ff00;
        header[24] = 0x0000_00ff;
        header[25] = 0xff00_0000;

        for value in header {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[0x38, 0x22, 0x0c, 0xff]);
        bytes
    }

    #[test]
    fn previews_the_uncompressed_dds_the_game_ships() {
        let directory = std::env::temp_dir().join("hoi4ms-asset-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("icon.dds");
        std::fs::write(&path, uncompressed_dds()).expect("write");

        let url = image_data_url(&path.display().to_string(), None).expect("encodes");
        let bytes = STANDARD
            .decode(url.trim_start_matches("data:image/png;base64,"))
            .expect("base64");
        let decoded = image::load_from_memory(&bytes).expect("png").to_rgba8();

        // Read back in the order the browser will draw it, not as stored.
        assert_eq!(decoded.get_pixel(0, 0), &image::Rgba([12, 34, 56, 255]));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn previews_a_png_a_mod_named_dds() {
        // The game reads the header rather than the name, and mods rely on it.
        let directory = std::env::temp_dir().join("hoi4ms-asset-tests");
        std::fs::create_dir_all(&directory).expect("temp dir");
        let path = directory.join("misnamed.dds");

        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([12, 34, 56, 255]));
        image
            .save_with_format(&path, ImageFormat::Png)
            .expect("save");

        let url = image_data_url(&path.display().to_string(), None).expect("encodes");
        assert!(url.starts_with("data:image/png;base64,"));

        std::fs::remove_file(&path).ok();
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
