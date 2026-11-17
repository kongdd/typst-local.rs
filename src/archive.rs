use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

pub fn package_dir(path: &Path) -> Result<PathBuf> {
    if path.join("typst.toml").is_file() {
        return Ok(path.to_owned());
    }
    let entries = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
    if entries.iter().any(|entry| {
        semver::Version::parse(&entry.file_name().to_string_lossy()).is_ok()
            && entry.path().join("typst.toml").is_file()
    }) {
        return Ok(path.to_owned());
    }
    if entries.len() == 1 && entries[0].file_type()?.is_dir() {
        return package_dir(&entries[0].path());
    }
    bail!("archive does not contain a package")
}

pub fn extract(path: &Path, destination: &Path) -> Result<()> {
    let file = fs::File::open(path)?;
    if path.to_string_lossy().ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.enclosed_name().context("unsafe archive path")?;
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                bail!("archive contains a symbolic link");
            }
            let target = destination.join(name);
            if entry.is_dir() {
                fs::create_dir_all(target)?;
            } else {
                fs::create_dir_all(target.parent().context("invalid archive path")?)?;
                std::io::copy(&mut entry, &mut fs::File::create(target)?)?;
            }
        }
    } else {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
        for entry in archive.entries()? {
            let mut entry = entry?;
            let kind = entry.header().entry_type();
            if !kind.is_file() && !kind.is_dir() {
                bail!("archive contains an unsupported entry");
            }
            if !entry.unpack_in(destination)? {
                bail!("unsafe archive path");
            }
        }
    }
    Ok(())
}
