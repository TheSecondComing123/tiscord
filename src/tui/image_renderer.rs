use crate::tui::terminal_caps::GraphicsProtocol;

/// Encode image bytes into terminal graphics protocol data.
/// Returns (protocol_data, width_cols, height_rows) or error.
pub fn encode_image(
    image_bytes: &[u8],
    protocol: GraphicsProtocol,
    max_width_cols: u16,
) -> Result<(Vec<u8>, u16, u16), String> {
    match protocol {
        GraphicsProtocol::Kitty => encode_kitty(image_bytes, max_width_cols),
        GraphicsProtocol::Sixel => Err("Sixel encoding not yet implemented".into()),
        GraphicsProtocol::None => Err("No graphics protocol available".into()),
    }
}

/// Encode image bytes using the Kitty graphics protocol.
///
/// The Kitty graphics protocol works by:
/// 1. Decoding the image from its source format
/// 2. Resizing to fit within the terminal column width (approximating 1 col ≈ 8px, 1 row ≈ 16px)
/// 3. Re-encoding as PNG bytes
/// 4. Base64-encoding the PNG
/// 5. Sending in 4096-byte chunks via escape sequences:
///    ESC_Gf=100,a=T,m={more};{chunk}ESC\
///    where m=1 means more chunks follow, m=0 means last chunk
fn encode_kitty(img_bytes: &[u8], max_width_cols: u16) -> Result<(Vec<u8>, u16, u16), String> {
    let img = image::load_from_memory(img_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?;

    // Resize to fit terminal width (approximate: 1 col ≈ 8px, 1 row ≈ 16px)
    let max_px = (max_width_cols as u32) * 8;
    let img = img.resize(max_px, max_px, image::imageops::FilterType::Lanczos3);

    // Encode as PNG
    let mut png_buf = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_buf),
        image::ImageFormat::Png,
    )
    .map_err(|e| format!("PNG encode failed: {e}"))?;

    // Base64 encode
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);

    // Build Kitty escape sequences in chunks
    let mut output = Vec::new();
    let chunk_size = 4096;
    let chunks: Vec<&[u8]> = b64.as_bytes().chunks(chunk_size).collect();
    for (i, chunk) in chunks.iter().enumerate() {
        let m = if i == chunks.len() - 1 { 0 } else { 1 };
        output.extend_from_slice(format!("\x1b_Gf=100,a=T,m={m};").as_bytes());
        output.extend_from_slice(chunk);
        output.extend_from_slice(b"\x1b\\");
    }

    let width_cols = (img.width() / 8).max(1) as u16;
    let height_rows = (img.height() / 16).max(1) as u16;
    Ok((output, width_cols, height_rows))
}

/// Check if a filename has an image extension
pub fn is_image_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".bmp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::terminal_caps::GraphicsProtocol;

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file("photo.png"));
        assert!(is_image_file("photo.PNG"));
        assert!(is_image_file("image.jpg"));
        assert!(is_image_file("image.jpeg"));
        assert!(is_image_file("anim.gif"));
        assert!(is_image_file("picture.webp"));
        assert!(is_image_file("bitmap.bmp"));
        assert!(!is_image_file("document.pdf"));
        assert!(!is_image_file("video.mp4"));
    }

    #[test]
    fn test_encode_image_none_protocol_returns_error() {
        let result = encode_image(b"fake", GraphicsProtocol::None, 40);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_image_sixel_not_implemented() {
        let result = encode_image(b"fake", GraphicsProtocol::Sixel, 40);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Sixel"));
    }

    #[test]
    fn test_encode_kitty_invalid_data_returns_error() {
        let result = encode_image(b"not an image", GraphicsProtocol::Kitty, 40);
        assert!(result.is_err());
    }

    /// Generate a minimal 1×1 PNG in memory to test Kitty encoding round-trip.
    #[test]
    fn test_encode_kitty_valid_png() {
        // Build a minimal 1×1 red PNG using the image crate itself.
        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(1, 1, |_, _| Rgb([255u8, 0, 0]));
        let mut png_buf = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_buf),
            image::ImageFormat::Png,
        )
        .unwrap();

        let result = encode_image(&png_buf, GraphicsProtocol::Kitty, 40);
        assert!(result.is_ok(), "encode_kitty failed: {:?}", result.err());
        let (data, width, height) = result.unwrap();
        assert!(!data.is_empty());
        assert!(width >= 1);
        assert!(height >= 1);
        // Verify the data starts with the Kitty escape sequence prefix
        let prefix = b"\x1b_G";
        assert!(
            data.starts_with(prefix),
            "output doesn't start with Kitty ESC sequence"
        );
    }
}
