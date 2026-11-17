use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::{archive, git, package};

pub fn from_source(root: &Path, source: &str) -> Result<()> {
    let path = Path::new(source);
    if path.is_dir() && !path.join(".git").exists() {
        return install(root, path);
    }
    let temp = tempfile::tempdir()?;
    let checkout = temp.path().join("package");
    if path.is_file() && (source.ends_with(".zip") || source.ends_with(".tar.gz")) {
        fs::create_dir(&checkout)?;
        archive::extract(path, &checkout)?;
        install(root, &archive::package_dir(&checkout)?)
    } else {
        git::clone(source, &checkout)?;
        install(root, &checkout)
    }
}

fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            fs::create_dir(&target)?;
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            bail!("unsupported file type: {}", entry.path().display());
        }
    }
    Ok(())
}

fn install(root: &Path, path: &Path) -> Result<()> {
    let source = path
        .canonicalize()
        .context("cannot access package directory")?;
    let (name, version) = identity(&source)?;

    fs::create_dir_all(root)?;
    if root.canonicalize()?.starts_with(&source) {
        bail!("package directory contains the local package directory");
    }
    let parent = root.join(&name);
    let destination = match &version {
        Some(version) => {
            fs::create_dir_all(&parent)?;
            parent.join(version)
        }
        None => parent,
    };
    fs::create_dir(&destination)
        .with_context(|| format!("cannot create {}", destination.display()))?;
    if let Err(error) = copy_dir(&source, &destination) {
        if let Err(cleanup) = fs::remove_dir_all(&destination) {
            eprintln!("failed to clean up {}: {cleanup}", destination.display());
        }
        return Err(error.context("failed to copy package"));
    }
    match version {
        Some(version) => println!("installed @local/{name}:{version}"),
        None => println!("installed @local/{name}"),
    }
    Ok(())
}

fn identity(source: &Path) -> Result<(String, Option<String>)> {
    let manifest = source.join("typst.toml");
    if manifest.is_file() {
        let package = package::parse_manifest(&fs::read_to_string(manifest)?)?;
        return Ok((package.name, Some(package.version)));
    }

    let mut name: Option<String> = None;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let version = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type()?.is_dir() || semver::Version::parse(&version).is_err() {
            continue;
        }
        let path = entry.path().join("typst.toml");
        let manifest =
            fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
        let package = package::parse_manifest(&manifest)?;
        if package.version != version || name.as_ref().is_some_and(|name| *name != package.name) {
            bail!("inconsistent package identity in {}", path.display());
        }
        name = Some(package.name);
    }
    Ok((name.context("no package manifest found")?, None))
}
