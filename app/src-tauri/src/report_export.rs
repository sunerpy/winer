//! A user-chosen report folder, remembered separately from game settings.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub(crate) fn directory(config: &Path) -> io::Result<Option<PathBuf>> {
    match fs::read(config.join("report-directory.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn remember(config: &Path, directory: &Path) -> io::Result<()> {
    fs::create_dir_all(config)?;
    let tmp = config.join("report-directory.tmp");
    fs::write(&tmp, serde_json::to_vec(directory)?)?;
    fs::rename(tmp, config.join("report-directory.json"))
}

pub(crate) fn write_png(directory: &Path, name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let stem = name
        .strip_suffix(".png")
        .ok_or_else(|| io::Error::other("Expected a PNG filename"))?;
    winer_core::export::check_stem(stem).map_err(io::Error::other)?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.len() > 64 * 1024 * 1024 {
        return Err(io::Error::other("Invalid or oversized PNG"));
    }
    // Do not silently recreate a moved/deleted destination, or overwrite previous reports.
    for copy in 1..=9999 {
        let name = if copy == 1 {
            format!("{stem}.png")
        } else {
            format!("{stem} ({copy}).png")
        };
        let path = directory.join(name);
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        return Ok(path);
    }
    Err(io::Error::other("Too many copies of this report"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remembers_folder_and_writes_without_overwriting_or_escaping() {
        let root = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\nexample";
        assert_eq!(directory(root.path()).unwrap(), None);
        remember(root.path(), root.path()).unwrap();
        assert_eq!(
            directory(root.path()).unwrap().as_deref(),
            Some(root.path())
        );
        let replacement = root.path().join("other-folder");
        remember(root.path(), &replacement).unwrap();
        assert_eq!(directory(root.path()).unwrap(), Some(replacement));
        let first = write_png(root.path(), "玩家.png", png).unwrap();
        let second = write_png(root.path(), "玩家.png", png).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(first).unwrap(), png);
        for bad in ["../escape.png", "C:\\escape.png", "CON.png", "wrong.zip"] {
            assert!(write_png(root.path(), bad, png).is_err());
        }
        assert!(write_png(root.path(), "bad.png", b"not PNG").is_err());
        assert!(write_png(&root.path().join("missing"), "good.png", png).is_err());
    }
}
