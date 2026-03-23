#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    None,
    Sixel,
    Kitty,
}

#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    pub graphics: GraphicsProtocol,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let graphics = detect_graphics_protocol();
        Self { graphics }
    }

    pub fn supports_images(&self) -> bool {
        self.graphics != GraphicsProtocol::None
    }
}

fn detect_graphics_protocol() -> GraphicsProtocol {
    // Check TERM_PROGRAM env var
    if let Ok(term) = std::env::var("TERM_PROGRAM") {
        match term.as_str() {
            "kitty" => return GraphicsProtocol::Kitty,
            "WezTerm" => return GraphicsProtocol::Kitty,
            "iTerm2" | "iTerm.app" => return GraphicsProtocol::Sixel,
            "mintty" => return GraphicsProtocol::Sixel,
            _ => {}
        }
    }
    // Check TERM for xterm with sixel
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("xterm") {
            // Some xterms support sixel
            if std::env::var("XTERM_VERSION").is_ok() {
                return GraphicsProtocol::Sixel;
            }
        }
    }
    GraphicsProtocol::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_runs_without_panic() {
        let caps = TerminalCapabilities::detect();
        let _ = caps.graphics;
    }

    #[test]
    fn test_supports_images() {
        let caps = TerminalCapabilities { graphics: GraphicsProtocol::Kitty };
        assert!(caps.supports_images());

        let caps = TerminalCapabilities { graphics: GraphicsProtocol::None };
        assert!(!caps.supports_images());
    }
}
