use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagesConfig {
    #[serde(default = "default_images_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_width")]
    pub max_width: u16,
}

fn default_images_enabled() -> bool { true }
fn default_max_width() -> u16 { 40 }

impl Default for ImagesConfig {
    fn default() -> Self {
        Self { enabled: default_images_enabled(), max_width: default_max_width() }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TimestampMode {
    Relative,
    Absolute,
    Off,
}

impl Default for TimestampMode {
    fn default() -> Self {
        TimestampMode::Relative
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiLayout {
    pub sidebar_width: u16,
    pub member_width: u16,
}

impl Default for UiLayout {
    fn default() -> Self {
        UiLayout {
            sidebar_width: 28,
            member_width: 24,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub fps: u16,
    pub timestamps: TimestampMode,
    pub member_sidebar: bool,
    pub layout: UiLayout,
}

impl Default for UiConfig {
    fn default() -> Self {
        UiConfig {
            fps: 30,
            timestamps: TimestampMode::default(),
            member_sidebar: true,
            layout: UiLayout::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub desktop: bool,
    pub mentions_only: bool,
    /// Play a terminal bell sound on mention.
    pub sound: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        NotificationConfig {
            desktop: false,
            mentions_only: false,
            sound: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionsConfig {
    #[serde(default = "default_recent_emojis")]
    pub recent: Vec<String>,
}

fn default_recent_emojis() -> Vec<String> {
    vec!["👍".into(), "❤️".into(), "😂".into(), "🔥".into(), "👀".into(), "🚀".into()]
}

impl Default for ReactionsConfig {
    fn default() -> Self {
        Self { recent: default_recent_emojis() }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ui: UiConfig,
    pub notifications: NotificationConfig,
    #[serde(default)]
    pub reactions: ReactionsConfig,
    #[serde(default)]
    pub images: ImagesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ui: UiConfig::default(),
            notifications: NotificationConfig::default(),
            reactions: ReactionsConfig::default(),
            images: ImagesConfig::default(),
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .expect("could not determine config directory")
            .join("tiscord")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .expect("could not determine data directory")
            .join("tiscord")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.ui.fps, 30);
        assert_eq!(config.ui.timestamps, TimestampMode::Relative);
        assert!(config.ui.member_sidebar);
        assert_eq!(config.ui.layout.sidebar_width, 28);
        assert_eq!(config.ui.layout.member_width, 24);

        assert!(!config.notifications.desktop);
        assert!(!config.notifications.mentions_only);
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
[ui]
fps = 60
"#;
        let config: Config = toml::from_str(toml_str).expect("failed to parse TOML");

        assert_eq!(config.ui.fps, 60);
        // Missing fields fall back to defaults
        assert_eq!(config.ui.timestamps, TimestampMode::Relative);
        assert!(config.ui.member_sidebar);
        assert_eq!(config.ui.layout.sidebar_width, 28);
        assert_eq!(config.ui.layout.member_width, 24);
        assert!(!config.notifications.desktop);
        assert!(!config.notifications.mentions_only);
    }
}
