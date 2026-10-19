//! Upload sanitization for client-supplied files.
//!
//! Client filenames are untrusted input: they may contain path traversal,
//! unsafe characters, or collide with another user's upload. Everything that
//! reaches Supabase Storage is therefore rewritten into a safe, per-user
//! object key of the form:
//!
//!     users/{user_id}/{uuid}-{sanitized_name}

use axum::http::StatusCode;
use std::fmt;
use uuid::Uuid;

/// Maximum accepted upload size (10 MiB).
pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 1600;
pub const JPEG_QUALITY: u8 = 80;

/// Maximum length of the sanitized filename portion.
const MAX_NAME_LEN: usize = 100;

/// Allowed image extensions (lowercase, without leading dot).
const ALLOWED_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];

#[derive(Debug)]
pub enum UploadError {
    /// Filename was empty or had no allowed extension.
    InvalidFilename(String),
    /// Content does not match a known image signature.
    UnsupportedContent,
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UploadError::InvalidFilename(msg) => write!(f, "invalid filename: {msg}"),
            UploadError::UnsupportedContent => {
                write!(
                    f,
                    "unsupported or invalid file content (allowed: jpg, jpeg, png, webp, gif)"
                )
            }
        }
    }
}

/// Sanitize an untrusted client filename into a safe object name.
///
/// - strips any directory components (`/` and `\`) — kills path traversal
/// - keeps only `[A-Za-z0-9-_]` in the name, replacing everything else with `_`
/// - requires one of the allowed image extensions (case-insensitive)
pub fn sanitize_filename(raw: &str) -> Result<String, UploadError> {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("").trim();

    if base.is_empty() {
        return Err(UploadError::InvalidFilename("filename is empty".into()));
    }

    let (name, ext) = match base.rfind('.') {
        Some(pos) => (&base[..pos], &base[pos + 1..]),
        None => (base, ""),
    };

    let ext = ext.to_ascii_lowercase();
    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(UploadError::InvalidFilename(format!(
            "extension '.{ext}' is not allowed"
        )));
    }

    let cleaned: String = name
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .take(MAX_NAME_LEN)
        .collect();

    let name = if cleaned.is_empty() { "image".to_string() } else { cleaned };

    Ok(format!("{name}.{ext}"))
}

/// Verify the bytes actually look like one of the allowed image formats
/// (magic-byte sniffing), so a renamed `.php`/`.html` payload is rejected
/// even if its extension passed the allowlist.
pub fn validate_image_content(bytes: &[u8]) -> Result<(), UploadError> {
    let ok = matches!(
        bytes,
        [0xFF, 0xD8, 0xFF, ..] // JPEG
            | [0x89, 0x50, 0x4E, 0x47, ..] // PNG
            | [0x47, 0x49, 0x46, 0x38, ..] // GIF ("GIF8")
            | [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] // WEBP
    );
    if ok {
        Ok(())
    } else {
        Err(UploadError::UnsupportedContent)
    }
}

/// Build a collision-free, per-user storage object key. The random UUID
/// prefix guarantees two uploads can never overwrite each other, even when
/// the same user re-uploads a file with an identical name.
pub fn build_storage_key(user_id: &Uuid, sanitized_name: &str) -> String {
    let uuid = Uuid::new_v4();
    format!("users/{user_id}/{uuid}-{sanitized_name}")
}

pub fn exif_orientation(data: &[u8]) -> u16 {
    use exif::{In, Reader, Tag, Value};
    let mut reader = std::io::BufReader::new(std::io::Cursor::new(data));
    match Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif
            .get_field(Tag::Orientation, In(1))
            .and_then(|f| match &f.value {
                Value::Short(v) => v.first().copied(),
                _ => None,
            })
            .unwrap_or(1),
        Err(_) => 1,
    }
}

pub fn apply_orientation(img: image::RgbaImage, orientation: u16) -> image::RgbaImage {
    use image::imageops;
    match orientation {
        2 => imageops::flip_horizontal(&img),
        3 => imageops::rotate180(&img),
        4 => imageops::flip_vertical(&img),
        5 => imageops::flip_horizontal(&imageops::rotate270(&img)),
        6 => imageops::rotate90(&img),
        7 => imageops::flip_horizontal(&imageops::rotate90(&img)),
        8 => imageops::rotate270(&img),
        _ => img,
    }
}

/// Normalizes an image: decodes raw bytes, applies EXIF orientation,
/// downscales to max 1600px with Lanczos3, and encodes to JPEG at quality 80.
#[allow(dead_code)]
pub fn process_and_encode_image(raw_bytes: &[u8]) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
    process_and_encode_image_matching_aspect(raw_bytes, 0, 0)
}

/// Maps dimensions to the closest aspect ratio string natively supported by Gemini image generation.
pub fn best_supported_aspect_ratio(w: u32, h: u32) -> &'static str {
    if w == 0 || h == 0 {
        return "1:1";
    }
    let ratio = w as f64 / h as f64;
    const SUPPORTED: &[(&str, f64)] = &[
        ("21:9", 21.0 / 9.0),
        ("16:9", 16.0 / 9.0),
        ("3:2", 3.0 / 2.0),
        ("4:3", 4.0 / 3.0),
        ("5:4", 5.0 / 4.0),
        ("1:1", 1.0 / 1.0),
        ("4:5", 4.0 / 5.0),
        ("3:4", 3.0 / 4.0),
        ("2:3", 2.0 / 3.0),
        ("9:16", 9.0 / 16.0),
    ];
    let mut best_str = "1:1";
    let mut min_diff = f64::MAX;
    for (name, val) in SUPPORTED {
        let diff = (ratio - val).abs();
        if diff < min_diff {
            min_diff = diff;
            best_str = name;
        }
    }
    best_str
}

/// Crops `img` to match the target aspect ratio (`target_w:target_h`),
/// symmetrically cropping from the center (excess width from left/right,
/// or excess height from top/bottom).
pub fn crop_to_aspect_ratio(
    img: image::RgbaImage,
    target_w: u32,
    target_h: u32,
) -> image::RgbaImage {
    if target_w == 0 || target_h == 0 {
        return img;
    }
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img;
    }

    let gen_ratio = w as f64 / h as f64;
    let target_ratio = target_w as f64 / target_h as f64;

    if (gen_ratio - target_ratio).abs() < 0.001 {
        return img;
    }

    if gen_ratio > target_ratio {
        let new_w = ((h as f64 * target_ratio).round() as u32).clamp(1, w);
        let offset_x = (w - new_w) / 2;
        image::imageops::crop_imm(&img, offset_x, 0, new_w, h).to_image()
    } else {
        let new_h = ((w as f64 / target_ratio).round() as u32).clamp(1, h);
        let offset_y = (h - new_h) / 2;
        image::imageops::crop_imm(&img, 0, offset_y, w, new_h).to_image()
    }
}

/// Decodes raw bytes, applies EXIF orientation, crops to match the target aspect ratio,
/// downscales to max 1600px, and encodes to JPEG at quality 80.
pub fn process_and_encode_image_matching_aspect(
    raw_bytes: &[u8],
    target_w: u32,
    target_h: u32,
) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
    let img = image::load_from_memory(raw_bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("failed to decode image: {e}")))?;

    let oriented = apply_orientation(img.to_rgba8(), exif_orientation(raw_bytes));
    let cropped = crop_to_aspect_ratio(oriented, target_w, target_h);

    let (w, h) = cropped.dimensions();
    let resized = if (w.max(h)) as f64 > MAX_IMAGE_DIMENSION as f64 {
        let scale = MAX_IMAGE_DIMENSION as f64 / (w.max(h)) as f64;
        image::imageops::resize(
            &cropped,
            (w as f64 * scale).round().max(1.0) as u32,
            (h as f64 * scale).round().max(1.0) as u32,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        cropped
    };

    let rgb = image::DynamicImage::ImageRgba8(resized).to_rgb8();
    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to encode jpeg: {e}")))?;

    Ok((out, "image/jpeg"))
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn strips_path_traversal() {
        assert_eq!(
            sanitize_filename("../../etc/passwd.png").unwrap(),
            "passwd.png"
        );
        assert_eq!(
            sanitize_filename("C:\\Users\\me\\photo.jpg").unwrap(),
            "photo.jpg"
        );
    }

    #[test]
    fn replaces_unsafe_characters() {
        assert_eq!(sanitize_filename("my photo (1).png").unwrap(), "my_photo__1_.png");
        assert_eq!(sanitize_filename("a\"b'c.png").unwrap(), "a_b_c.png");
    }

    #[test]
    fn rejects_disallowed_extensions() {
        assert!(matches!(
            sanitize_filename("evil.php"),
            Err(UploadError::InvalidFilename(_))
        ));
        assert!(sanitize_filename("noext").is_err());
        assert!(sanitize_filename(".").is_err());
    }

    #[test]
    fn extension_check_is_case_insensitive() {
        assert_eq!(sanitize_filename("PHOTO.JPG").unwrap(), "PHOTO.jpg");
    }

    #[test]
    fn caps_long_names() {
        let long = format!("{}x.png", "a".repeat(500));
        let out = sanitize_filename(&long).unwrap();
        assert!(out.len() <= MAX_NAME_LEN + 4);
    }

    #[test]
    fn handles_dot_only_names() {
        assert_eq!(sanitize_filename(".png").unwrap(), "image.png");
    }

    #[test]
    fn validates_magic_bytes() {
        assert!(validate_image_content(&[0xFF, 0xD8, 0xFF, 0xE0]).is_ok()); // JPEG
        assert!(validate_image_content(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x1A]).is_ok()); // PNG
        assert!(validate_image_content(b"GIF89a").is_ok()); // GIF
        assert!(validate_image_content(b"RIFF\x00\x00\x00\x00WEBP").is_ok()); // WEBP
        assert!(matches!(
            validate_image_content(b"<html>"),
            Err(UploadError::UnsupportedContent)
        ));
        assert!(validate_image_content(&[]).is_err());
    }

    #[test]
    fn keys_are_per_user_and_unique() {
        let user = Uuid::new_v4();
        let a = build_storage_key(&user, "a.png");
        let b = build_storage_key(&user, "a.png");
        assert_ne!(a, b);
        assert!(a.starts_with(&format!("users/{user}/")));
    }

    #[test]
    fn test_best_supported_aspect_ratio() {
        assert_eq!(best_supported_aspect_ratio(3024, 4032), "3:4");
        assert_eq!(best_supported_aspect_ratio(1080, 1920), "9:16");
        assert_eq!(best_supported_aspect_ratio(1000, 1500), "2:3");
        assert_eq!(best_supported_aspect_ratio(1080, 1350), "4:5");
        assert_eq!(best_supported_aspect_ratio(1000, 1000), "1:1");
        assert_eq!(best_supported_aspect_ratio(4032, 3024), "4:3");
        assert_eq!(best_supported_aspect_ratio(1920, 1080), "16:9");
        assert_eq!(best_supported_aspect_ratio(0, 0), "1:1");
    }

    #[test]
    fn test_crop_to_aspect_ratio() {
        let img = image::RgbaImage::new(1000, 1000); // 1:1
        // Crop to 3:4 target (width should become 750, height stays 1000)
        let cropped = crop_to_aspect_ratio(img.clone(), 3, 4);
        assert_eq!(cropped.dimensions(), (750, 1000));

        // Crop to 9:16 target (width should become 563, height stays 1000)
        let cropped916 = crop_to_aspect_ratio(img.clone(), 9, 16);
        assert_eq!(cropped916.dimensions(), (563, 1000));

        // Crop to 16:9 target (width stays 1000, height becomes 563)
        let cropped169 = crop_to_aspect_ratio(img.clone(), 16, 9);
        assert_eq!(cropped169.dimensions(), (1000, 563));

        // Matching aspect ratio needs no crop
        let matching = crop_to_aspect_ratio(img.clone(), 1, 1);
        assert_eq!(matching.dimensions(), (1000, 1000));
    }
}
