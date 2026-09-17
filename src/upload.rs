//! Upload sanitization for client-supplied files.
//!
//! Client filenames are untrusted input: they may contain path traversal,
//! unsafe characters, or collide with another user's upload. Everything that
//! reaches Supabase Storage is therefore rewritten into a safe, per-user
//! object key of the form:
//!
//!     users/{user_id}/{uuid}-{sanitized_name}

use std::fmt;
use uuid::Uuid;

/// Maximum accepted upload size (10 MiB).
pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

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
}
