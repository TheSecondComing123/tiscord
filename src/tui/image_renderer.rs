use crate::tui::terminal_caps::GraphicsProtocol;

/// Encode image bytes into terminal graphics protocol data.
/// Returns (protocol_data, width_cols, height_rows) or error.
pub fn encode_image(
    _image_bytes: &[u8],
    protocol: GraphicsProtocol,
    _max_width_cols: u16,
) -> Result<(Vec<u8>, u16, u16), String> {
    match protocol {
        GraphicsProtocol::Kitty => {
            // TODO: Implement Kitty graphics protocol encoding
            // Requires: image crate for decoding, base64 for encoding
            // Kitty protocol: ESC_G payload ESC_\ with chunked base64 PNG
            Err("Kitty image encoding not yet implemented - add 'image' and 'base64' crates".into())
        }
        GraphicsProtocol::Sixel => {
            // TODO: Implement Sixel encoding
            // Requires: image crate, color quantization to 256 colors
            Err("Sixel image encoding not yet implemented".into())
        }
        GraphicsProtocol::None => {
            Err("No graphics protocol available".into())
        }
    }
}

/// Check if a filename has an image extension
pub fn is_image_file(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg")
        || lower.ends_with(".gif") || lower.ends_with(".webp") || lower.ends_with(".bmp")
}
