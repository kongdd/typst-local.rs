use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::package;

fn run(path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .context("cannot run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn clone(source: &str, destination: &Path) -> Result<()> {
    let path = Path::new(source);
    let source = if path.exists() {
        path.canonicalize()?.to_string_lossy().into_owned()
    } else {
        source.to_owned()
    };
    let status = Command::new("git")
        .args(["clone", "--", &source])
        .arg(destination)
        .status()
        .context("cannot run git")?;
    if !status.success() {
        bail!("git clone failed");
    }
    Ok(())
}

fn repositories(path: &Path) -> Result<Vec<PathBuf>> {
    if path.join(".git").exists() {
        return Ok(vec![path.to_owned()]);
    }
    let mut repositories = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && semver::Version::parse(&entry.file_name().to_string_lossy()).is_ok()
            && entry.path().join(".git").exists()
        {
            repositories.push(entry.path());
        }
    }
    repositories.sort();
    Ok(repositories)
}

fn versions(path: &Path) -> Result<Vec<semver::Version>> {
    let mut versions = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Ok(version) = semver::Version::parse(&entry.file_name().to_string_lossy()) {
                versions.push(version);
            }
        }
    }
    if versions.is_empty() {
        let manifest = path.join("typst.toml");
        if manifest.is_file() {
            let package = package::parse_manifest(&fs::read_to_string(manifest)?)?;
            versions.push(semver::Version::parse(&package.version)?);
        }
    }
    versions.sort();
    Ok(versions)
}

fn update_repository(repo: &Path, expected: Option<(&str, &str)>) -> Result<String> {
    let before = run(repo, &["rev-parse", "HEAD"])?;
    let old_versions = versions(repo)?;
    if let Some((name, version)) = expected {
        run(repo, &["fetch"])?;
        let manifest = run(repo, &["show", "@{upstream}:typst.toml"])?;
        let upstream = package::parse_manifest(&manifest)?;
        if upstream.name != name || upstream.version != version {
            bail!("upstream package identity changed; install the new version separately");
        }
    }
    run(repo, &["pull", "--ff-only"])?;
    let commits = run(repo, &["rev-list", "--count", &format!("{before}..HEAD")])?;
    let new_versions = versions(repo)?;
    let changes: Vec<_> = new_versions
        .iter()
        .filter(|version| !old_versions.contains(version))
        .map(|version| format!("+{version}"))
        .chain(
            old_versions
                .iter()
                .filter(|version| !new_versions.contains(version))
                .map(|version| format!("-{version}")),
        )
        .collect();
    let version_change = if old_versions.is_empty() && new_versions.is_empty() {
        "versions unknown".to_owned()
    } else if changes.is_empty() {
        "versions unchanged".to_owned()
    } else {
        format!("versions {}", changes.join(", "))
    };
    let latest = new_versions
        .last()
        .map_or_else(|| "unknown".to_owned(), ToString::to_string);
    Ok(format!(
        "latest {latest}  ·  {commits} commits  ·  {version_change}"
    ))
}

fn label(name: &str, package_path: &Path, repo: &Path) -> String {
    if repo == package_path {
        name.to_owned()
    } else {
        format!(
            "{name}:{}",
            repo.file_name().unwrap_or_default().to_string_lossy()
        )
    }
}

fn label_width(root: &Path, names: &[String]) -> Result<usize> {
    let mut width = 0;
    for name in names {
        let path = root.join(name);
        width = width.max(name.len());
        for repo in repositories(&path)? {
            width = width.max(label(name, &path, &repo).len());
        }
    }
    Ok(width)
}

pub fn update(root: &Path, name: &str) -> Result<()> {
    package::validate(name, "0.0.0")?;
    let path = root.join(name);
    if !path.is_dir() {
        bail!("package not found: {name}");
    }
    let width = label_width(root, &[name.to_owned()])?;
    update_package(root, name, width)
}

fn update_package(root: &Path, name: &str, width: usize) -> Result<()> {
    package::validate(name, "0.0.0")?;
    let path = root.join(name);
    if !path.is_dir() {
        bail!("package not found: {name}");
    }
    let repos = repositories(&path)?;
    if repos.is_empty() {
        println!("{name:width$}  skipped (not a Git repository)");
        return Ok(());
    }
    let mut failed = false;
    for repo in repos {
        let label = label(name, &path, &repo);
        let version = repo
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let expected = (repo != path).then_some((name, version));
        match update_repository(&repo, expected) {
            Ok(summary) => println!("{label:width$}  {summary}"),
            Err(error) => {
                eprintln!("failed to update {}: {error:#}", repo.display());
                failed = true;
            }
        }
    }
    if failed {
        bail!("one or more repositories failed to update");
    }
    Ok(())
}

pub fn update_all(root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() && package::validate(&name, "0.0.0").is_ok() {
            names.push(name);
        }
    }
    names.sort();
    let width = label_width(root, &names)?;
    let mut failed = false;
    for name in names {
        if let Err(error) = update_package(root, &name, width) {
            eprintln!("failed to update @local/{name}: {error:#}");
            failed = true;
        }
    }
    if failed {
        bail!("one or more packages failed to update");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_package_or_version_repositories() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("example");
        fs::create_dir_all(package.join("0.1.0/.git")).unwrap();
        fs::create_dir_all(package.join("0.2.0")).unwrap();
        fs::create_dir_all(package.join("tests/.git")).unwrap();
        assert_eq!(repositories(&package).unwrap(), vec![package.join("0.1.0")]);
        fs::create_dir(package.join(".git")).unwrap();
        assert_eq!(repositories(&package).unwrap(), vec![package]);
    }
}
