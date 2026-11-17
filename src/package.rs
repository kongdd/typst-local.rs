use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    package: Package,
}

#[derive(Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

pub fn parse_manifest(text: &str) -> Result<Package> {
    let package = toml::from_str::<Manifest>(text)
        .context("invalid typst.toml")?
        .package;
    validate(&package.name, &package.version)?;
    Ok(package)
}

pub fn root() -> Result<PathBuf> {
    let base = match std::env::var_os("TYPST_PACKAGE_PATH") {
        Some(path) => PathBuf::from(path),
        None => dirs::data_dir()
            .context("cannot determine the system data directory")?
            .join("typst/packages"),
    };
    Ok(base.join("local"))
}

pub fn validate(name: &str, version: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("invalid package name: {name}");
    }
    semver::Version::parse(version).context("invalid package version")?;
    Ok(())
}

pub fn directory(root: &Path, name: &str) -> Result<PathBuf> {
    validate(name, "0.0.0")?;
    let path = root.join(name);
    if !path.is_dir() {
        bail!("package directory not found: {}", path.display());
    }
    path.canonicalize()
        .context("cannot resolve package directory")
}

fn installed(root: &Path) -> Result<Vec<(String, Vec<semver::Version>)>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut packages = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let mut versions = Vec::new();
        for version in fs::read_dir(entry.path())? {
            let version = version?;
            if version.file_type()?.is_dir() {
                if let Ok(number) = semver::Version::parse(&version.file_name().to_string_lossy()) {
                    versions.push(number);
                }
            }
        }
        if !versions.is_empty() {
            versions.sort();
            packages.push((entry.file_name().to_string_lossy().into_owned(), versions));
        }
    }
    packages.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(packages)
}

pub fn list(root: &Path) -> Result<()> {
    for (name, versions) in installed(root)? {
        let versions = versions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("@local/{name}: {versions}");
    }
    Ok(())
}

pub fn uninstall(root: &Path, name: &str, version: &str) -> Result<()> {
    validate(name, version)?;
    let path = root.join(name).join(version);
    fs::remove_dir_all(&path).with_context(|| format!("cannot remove {}", path.display()))?;
    println!("uninstalled @local/{name}:{version}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_package_identity() {
        assert!(validate("my-package", "1.2.3").is_ok());
        assert!(validate("../package", "1.2.3").is_err());
        assert!(validate("package", "../1.2.3").is_err());
    }

    #[test]
    fn lists_only_versions_grouped_by_package() {
        let temp = tempfile::tempdir().unwrap();
        for dir in [
            "example/0.1.10",
            "example/0.1.2",
            "example/.git",
            "example/tests",
        ] {
            fs::create_dir_all(temp.path().join(dir)).unwrap();
        }
        let packages = installed(temp.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].0, "example");
        assert_eq!(
            packages[0].1,
            vec![
                semver::Version::new(0, 1, 2),
                semver::Version::new(0, 1, 10)
            ]
        );
    }

    #[test]
    fn parses_manifest() {
        let package = parse_manifest("[package]\nname = 'example'\nversion = '1.0.0'").unwrap();
        assert_eq!(
            (package.name.as_str(), package.version.as_str()),
            ("example", "1.0.0")
        );
    }
}
