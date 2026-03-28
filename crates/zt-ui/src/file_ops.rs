use anyhow::{Context, Result};
use std::path::Path;

/// Rename a file within the vault. `new_name` is the bare filename (with extension).
/// Returns the new `rel_path` on success.
pub fn rename_file(vault_root: &Path, rel_path: &str, new_name: &str) -> Result<String> {
    let old_full = vault_root.join(rel_path);
    let parent = Path::new(rel_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_string_lossy().into_owned());
    let new_rel = match parent {
        Some(p) => format!("{}/{}", p, new_name),
        None => new_name.to_string(),
    };
    let new_full = vault_root.join(&new_rel);
    std::fs::rename(&old_full, &new_full)
        .with_context(|| format!("rename {} → {}", old_full.display(), new_full.display()))?;
    Ok(new_rel)
}

/// Delete a file from the vault.
pub fn delete_file(vault_root: &Path, rel_path: &str) -> Result<()> {
    let full = vault_root.join(rel_path);
    std::fs::remove_file(&full)
        .with_context(|| format!("delete {}", full.display()))
}

/// Create a new `.typ` file. `dir` is relative to vault root (empty = vault root).
/// Returns the new `rel_path`.
pub fn create_file(vault_root: &Path, dir: &str, name: &str) -> Result<String> {
    let file_name = if name.ends_with(".typ") {
        name.to_string()
    } else {
        format!("{}.typ", name)
    };
    let rel = if dir.is_empty() {
        file_name.clone()
    } else {
        format!("{}/{}", dir, file_name)
    };
    let full = vault_root.join(&rel);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, "")
        .with_context(|| format!("create {}", full.display()))?;
    Ok(rel)
}

/// Create a directory within the vault.
pub fn create_folder(vault_root: &Path, parent: &str, name: &str) -> Result<()> {
    let rel = if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent, name)
    };
    std::fs::create_dir_all(vault_root.join(&rel))
        .with_context(|| format!("mkdir {}", rel))
}

/// Move a file to a different directory within the vault.
/// Returns the new `rel_path`.
pub fn move_file(vault_root: &Path, from_rel: &str, to_dir: &str) -> Result<String> {
    let file_name = Path::new(from_rel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(from_rel);
    let new_rel = if to_dir.is_empty() {
        file_name.to_string()
    } else {
        format!("{}/{}", to_dir, file_name)
    };
    let old_full = vault_root.join(from_rel);
    let new_full = vault_root.join(&new_rel);
    if let Some(p) = new_full.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::rename(&old_full, &new_full)
        .with_context(|| format!("move {} → {}", old_full.display(), new_full.display()))?;
    Ok(new_rel)
}

/// Append a chapter entry to `.zetteltypsten/book.toml`.
pub fn add_to_book(vault_root: &Path, rel_path: &str) -> Result<()> {
    let toml_path = vault_root.join(".zetteltypsten/book.toml");
    let stem = Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(rel_path);
    let addition = format!(
        "\n[[suffix_chapters]]\ntitle = \"{}\"\nfile = \"{}\"\n",
        stem, rel_path
    );
    if toml_path.exists() {
        let content = std::fs::read_to_string(&toml_path)?;
        let updated = format!("{}\n{}", content.trim_end(), addition.trim_start());
        std::fs::write(&toml_path, updated)?;
    } else {
        if let Some(parent) = toml_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &toml_path,
            format!(
                "[book]\ntitle = \"My Book\"\n\nsrc = \".\"\n{}",
                addition.trim_start()
            ),
        )?;
    }
    Ok(())
}
