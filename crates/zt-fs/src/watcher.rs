use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Events emitted by the vault file watcher.
#[derive(Clone, Debug)]
pub enum VaultEvent {
    /// A file was created.
    Created(PathBuf),
    /// A file was modified.
    Modified(PathBuf),
    /// A file was deleted.
    Deleted(PathBuf),
    /// A file was renamed (from, to).
    Renamed(PathBuf, PathBuf),
}

/// Watches a vault directory for filesystem changes.
pub struct VaultWatcher {
    _watcher: RecommendedWatcher,
    receiver: mpsc::Receiver<VaultEvent>,
}

impl VaultWatcher {
    /// Start watching the given directory recursively.
    pub fn new(root: &Path) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let events = translate_event(event);
                for evt in events {
                    let _ = tx.send(evt);
                }
            }
        })?;

        watcher.watch(root, RecursiveMode::Recursive)?;

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }

    /// Try to receive pending events (non-blocking).
    pub fn try_recv(&self) -> Vec<VaultEvent> {
        let mut events = Vec::new();
        while let Ok(evt) = self.receiver.try_recv() {
            events.push(evt);
        }
        events
    }
}

fn translate_event(event: Event) -> Vec<VaultEvent> {
    let mut results = Vec::new();
    let paths = event.paths;

    match event.kind {
        EventKind::Create(_) => {
            for p in paths {
                results.push(VaultEvent::Created(p));
            }
        }
        EventKind::Modify(_) => {
            for p in paths {
                results.push(VaultEvent::Modified(p));
            }
        }
        EventKind::Remove(_) => {
            for p in paths {
                results.push(VaultEvent::Deleted(p));
            }
        }
        _ => {}
    }

    results
}
