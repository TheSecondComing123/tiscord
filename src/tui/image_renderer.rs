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
        GraphicsProtocol::Sixel => encode_sixel(image_bytes, max_width_cols),
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

/// Encode image bytes using the Sixel graphics protocol.
///
/// Sixel works by:
/// 1. Decoding and resizing the image
/// 2. Quantizing to a 256-color palette
/// 3. Encoding each 6-row band as sixel characters
///
/// Format: ESC P q <palette definitions> <sixel data> ESC \
fn encode_sixel(img_bytes: &[u8], max_width_cols: u16) -> Result<(Vec<u8>, u16, u16), String> {
    let img = image::load_from_memory(img_bytes)
        .map_err(|e| format!("failed to decode image: {e}"))?;

    // Resize to fit terminal width (approximate: 1 col ≈ 8px, 1 row ≈ 16px)
    let max_px = (max_width_cols as u32) * 8;
    let img = img.resize(max_px, max_px, image::imageops::FilterType::Lanczos3);
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width() as usize, rgba.height() as usize);

    // Quantize: build a simple palette by sampling unique colors (capped at 256).
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut pixel_indices = vec![0u8; w * h];
    for (i, pixel) in rgba.pixels().enumerate() {
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let idx = if let Some(pos) = palette.iter().position(|c| *c == rgb) {
            pos
        } else if palette.len() < 256 {
            palette.push(rgb);
            palette.len() - 1
        } else {
            // Find closest color in palette (simple Euclidean distance).
            palette
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| {
                    let dr = c[0] as i32 - rgb[0] as i32;
                    let dg = c[1] as i32 - rgb[1] as i32;
                    let db = c[2] as i32 - rgb[2] as i32;
                    dr * dr + dg * dg + db * db
                })
                .map(|(i, _)| i)
                .unwrap_or(0)
        };
        pixel_indices[i] = idx as u8;
    }

    let mut output = Vec::new();
    // DCS q (start sixel, default aspect ratio)
    output.extend_from_slice(b"\x1bPq");

    // Define palette: #idx;2;r%;g%;b%
    for (i, color) in palette.iter().enumerate() {
        let r_pct = (color[0] as u32 * 100) / 255;
        let g_pct = (color[1] as u32 * 100) / 255;
        let b_pct = (color[2] as u32 * 100) / 255;
        output.extend_from_slice(format!("#{};2;{};{};{}", i, r_pct, g_pct, b_pct).as_bytes());
    }

    // Encode sixel data: process 6 rows at a time
    let bands = (h + 5) / 6;
    for band in 0..bands {
        let y_start = band * 6;
        // For each color used in this band, emit a row of sixel characters
        for color_idx in 0..palette.len() {
            let ci = color_idx as u8;
            let mut has_pixel = false;
            let mut sixel_row = Vec::with_capacity(w);
            for x in 0..w {
                let mut sixel_val: u8 = 0;
                for bit in 0..6 {
                    let y = y_start + bit;
                    if y < h && pixel_indices[y * w + x] == ci {
                        sixel_val |= 1 << bit;
                        has_pixel = true;
                    }
                }
                sixel_row.push(sixel_val + 0x3F); // Sixel char = value + 63
            }
            if has_pixel {
                // Select color and emit the sixel row
                output.extend_from_slice(format!("#{}", color_idx).as_bytes());
                output.extend_from_slice(&sixel_row);
                output.push(b'$'); // Carriage return (stay on same band)
            }
        }
        output.push(b'-'); // New line (next band)
    }

    // String terminator
    output.extend_from_slice(b"\x1b\\");

    let width_cols = (w / 8).max(1) as u16;
    let height_rows = (h / 16).max(1) as u16;
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
    fn test_encode_sixel_valid_png() {
        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(2, 2, |x, y| {
            if (x + y) % 2 == 0 { Rgb([255u8, 0, 0]) } else { Rgb([0u8, 0, 255]) }
        });
        let mut png_buf = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_buf),
            image::ImageFormat::Png,
        )
        .unwrap();

        let result = encode_image(&png_buf, GraphicsProtocol::Sixel, 40);
        assert!(result.is_ok(), "encode_sixel failed: {:?}", result.err());
        let (data, width, height) = result.unwrap();
        assert!(!data.is_empty());
        assert!(width >= 1);
        assert!(height >= 1);
        // Verify the data starts with the Sixel DCS prefix
        assert!(
            data.starts_with(b"\x1bPq"),
            "output doesn't start with Sixel DCS sequence"
        );
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
