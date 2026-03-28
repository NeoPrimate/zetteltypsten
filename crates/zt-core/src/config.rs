use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Global application configuration, stored at `~/.config/zetteltypsten/config.toml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default)]
    pub recent_vaults: Vec<PathBuf>,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub preview: PreviewConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "default_tab_size")]
    pub tab_size: u32,
    #[serde(default = "default_true")]
    pub soft_wrap: bool,
    #[serde(default = "default_true")]
    pub line_numbers: bool,
    #[serde(default)]
    pub vim_mode: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewConfig {
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default = "default_page_width")]
    pub page_width_pt: f64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            recent_vaults: Vec::new(),
            editor: EditorConfig::default(),
            preview: PreviewConfig::default(),
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: default_tab_size(),
            soft_wrap: true,
            line_numbers: true,
            vim_mode: false,
        }
    }
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            auto_refresh: true,
            debounce_ms: default_debounce_ms(),
            page_width_pt: default_page_width(),
        }
    }
}

fn default_theme() -> String { "dark".into() }
fn default_font_family() -> String { "JetBrains Mono".into() }
fn default_font_size() -> f32 { 14.0 }
fn default_tab_size() -> u32 { 2 }
fn default_true() -> bool { true }
fn default_debounce_ms() -> u64 { 300 }
fn default_page_width() -> f64 { 595.28 } // A4 width in points
