use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A file or directory entry in the vault.
#[derive(Clone, Debug)]
pub struct VaultEntry {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to vault root.
    pub rel_path: PathBuf,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Display name (filename without extension for .typ files).
    pub name: String,
}

impl VaultEntry {
    pub fn is_typ_file(&self) -> bool {
        !self.is_dir
            && self
                .path
                .extension()
                .is_some_and(|ext| ext == "typ")
    }
}

/// Scan a vault directory and return all entries.
///
/// Excludes hidden directories and the `.zetteltypsten` metadata folder.
pub fn scan_vault(root: &Path) -> Result<Vec<VaultEntry>> {
    let mut entries = Vec::new();

    for entry in WalkDir::new(root)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| !is_excluded(e))
    {
        let entry = entry?;
        let path = entry.path().to_path_buf();
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_path_buf();
        let is_dir = entry.file_type().is_dir();
        let name = path
            .file_stem()
            .or_else(|| path.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        entries.push(VaultEntry {
            path,
            rel_path,
            is_dir,
            name,
        });
    }

    Ok(entries)
}

/// Build a flat list of only `.typ` files in the vault.
pub fn scan_typ_files(root: &Path) -> Result<Vec<VaultEntry>> {
    Ok(scan_vault(root)?
        .into_iter()
        .filter(|e| e.is_typ_file())
        .collect())
}

fn is_excluded(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    // Exclude hidden files/dirs and .zetteltypsten metadata
    name.starts_with('.') || name == "node_modules" || name == "target"
}

/// Represents the vault directory as a tree structure.
#[derive(Clone, Debug)]
pub struct TreeNode {
    pub entry: VaultEntry,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
}

impl TreeNode {
    /// Build a tree from a flat list of vault entries.
    pub fn build_tree(root: &Path) -> Result<Vec<TreeNode>> {
        let entries = scan_vault(root)?;
        let mut root_children: Vec<TreeNode> = Vec::new();

        // Separate directories and files at each level
        let mut dirs: Vec<&VaultEntry> = Vec::new();
        let mut files: Vec<&VaultEntry> = Vec::new();

        for entry in &entries {
            // Only include top-level entries (depth 1)
            if entry.rel_path.components().count() == 1 {
                if entry.is_dir {
                    dirs.push(entry);
                } else {
                    files.push(entry);
                }
            }
        }

        // Add directories first (sorted), then files (sorted)
        for dir in &dirs {
            let children = build_children(root, &dir.rel_path, &entries);
            root_children.push(TreeNode {
                entry: (*dir).clone(),
                children,
                expanded: false,
            });
        }

        for file in &files {
            root_children.push(TreeNode {
                entry: (*file).clone(),
                children: Vec::new(),
                expanded: false,
            });
        }

        Ok(root_children)
    }
}

fn build_children(root: &Path, parent_rel: &Path, all: &[VaultEntry]) -> Vec<TreeNode> {
    let mut dirs: Vec<&VaultEntry> = Vec::new();
    let mut files: Vec<&VaultEntry> = Vec::new();

    for entry in all {
        if let Some(parent) = entry.rel_path.parent() {
            if parent == parent_rel && entry.rel_path != parent_rel {
                if entry.is_dir {
                    dirs.push(entry);
                } else {
                    files.push(entry);
                }
            }
        }
    }

    let mut children = Vec::new();

    for dir in dirs {
        let grandchildren = build_children(root, &dir.rel_path, all);
        children.push(TreeNode {
            entry: dir.clone(),
            children: grandchildren,
            expanded: false,
        });
    }

    for file in files {
        children.push(TreeNode {
            entry: file.clone(),
            children: Vec::new(),
            expanded: false,
        });
    }

    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_temp_vault() {
        let tmp = std::env::temp_dir().join("zt_test_vault");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("notes")).unwrap();
        fs::write(tmp.join("index.typ"), "= Index").unwrap();
        fs::write(tmp.join("notes/hello.typ"), "= Hello").unwrap();
        fs::write(tmp.join(".hidden"), "secret").unwrap();

        let entries = scan_vault(&tmp).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"index"), "Should find index.typ");
        assert!(names.contains(&"hello"), "Should find notes/hello.typ");
        assert!(!names.iter().any(|n| n.starts_with('.')), "Should exclude hidden");

        let _ = fs::remove_dir_all(&tmp);
    }
}
