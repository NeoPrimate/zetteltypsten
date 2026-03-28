use crate::note::{Note, NoteId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Persisted vault configuration, stored in `<vault>/.zetteltypsten/vault.toml`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultConfig {
    pub root_path: PathBuf,
    pub name: String,
    #[serde(default = "default_daily_folder")]
    pub daily_notes_folder: String,
    #[serde(default = "default_templates_folder")]
    pub templates_folder: String,
    #[serde(default)]
    pub excluded_patterns: Vec<String>,
}

fn default_daily_folder() -> String {
    "daily".into()
}

fn default_templates_folder() -> String {
    "templates".into()
}

impl VaultConfig {
    pub fn new(root_path: PathBuf) -> Self {
        let name = root_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Vault".into());
        Self {
            root_path,
            name,
            daily_notes_folder: default_daily_folder(),
            templates_folder: default_templates_folder(),
            excluded_patterns: vec![".zetteltypsten".into()],
        }
    }
}

/// Runtime vault state (not persisted).
pub struct Vault {
    pub config: VaultConfig,
    pub notes: HashMap<NoteId, Note>,
}

impl Vault {
    pub fn new(config: VaultConfig) -> Self {
        Self {
            config,
            notes: HashMap::new(),
        }
    }
}
