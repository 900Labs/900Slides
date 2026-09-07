//! Durable local document identity and file replacement.

use std::{fs, io::Write, path::Path};

use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStatus {
    pub path: Option<String>,
    pub is_dirty: bool,
}

impl DocumentStatus {
    pub fn check_replace(&self, discard_changes: bool) -> Result<(), String> {
        if self.is_dirty && !discard_changes {
            Err("Save or discard the current presentation before replacing it".into())
        } else {
            Ok(())
        }
    }
}

/// Stage and sync a complete sibling file before replacing the destination.
/// A unique, exclusively created temporary file cannot overwrite another save.
pub fn atomic_save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    atomic_save_with(path, |file| file.write_all(bytes))
}

fn atomic_save_with(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = parent.join(format!(".900slides-save-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        write(&mut file)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok::<(), std::io::Error>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("slides-save-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn partial_write_failure_preserves_existing_presentation_and_cleans_staging() {
        let dir = test_dir();
        let path = dir.join("lesson.pptx");
        fs::write(&path, b"original complete presentation").unwrap();
        let result = atomic_save_with(&path, |file| {
            file.write_all(b"partial")?;
            Err(std::io::Error::other("simulated full disk"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original complete presentation");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_replacement_preserves_destination_and_cleans_staging() {
        let dir = test_dir();
        let target = dir.join("lesson.pptx");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"existing data").unwrap();
        assert!(atomic_save(&target, b"new bytes").is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"existing data");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn successful_save_replaces_complete_document() {
        let dir = test_dir();
        let path = dir.join("lesson.pptx");
        fs::write(&path, b"old").unwrap();
        atomic_save(&path, b"complete replacement").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete replacement");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replacing_unsaved_document_requires_explicit_discard() {
        let status = DocumentStatus {
            path: None,
            is_dirty: true,
        };
        assert!(status.check_replace(false).is_err());
        assert!(status.check_replace(true).is_ok());
        assert!(DocumentStatus::default().check_replace(false).is_ok());
    }
}
