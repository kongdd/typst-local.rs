use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repository(path: &Path) {
    fs::create_dir_all(path).unwrap();
    git(path, &["init", "-q"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@example.com"]);
}

fn commit(path: &Path) {
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "test"]);
}

fn manifest(path: &Path, version: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("typst.toml"),
        format!("[package]\nname = 'example'\nversion = '{version}'\nentrypoint = 'lib.typ'\n"),
    )
    .unwrap();
    fs::write(path.join("lib.typ"), "// example\n").unwrap();
}

fn typ(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_typ"))
        .env("TYPST_PACKAGE_PATH", root)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn installs_and_updates_multi_version_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let root = temp.path().join("packages");
    repository(&repo);
    manifest(&repo.join("0.1.0"), "0.1.0");
    commit(&repo);
    success(typ(&root, &["install", repo.to_str().unwrap()]));
    assert!(!typ(&root, &["install", repo.to_str().unwrap()])
        .status
        .success());
    assert!(root.join("local/example/.git").is_dir());
    assert!(success(typ(&root, &["list"])).contains("@local/example: 0.1.0"));

    manifest(&repo.join("0.2.0"), "0.2.0");
    commit(&repo);
    let output = success(typ(&root, &["update", "--all"]));
    assert!(output.contains("1 commits"));
    assert!(output.contains("+0.2.0"));
    assert!(root.join("local/example/0.2.0/typst.toml").is_file());
    assert!(success(typ(&root, &["update", "example"])).contains("0 commits"));
}

#[test]
fn refuses_version_change_in_version_directory() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let root = temp.path().join("packages");
    repository(&repo);
    manifest(&repo, "0.1.0");
    commit(&repo);
    success(typ(&root, &["install", repo.to_str().unwrap()]));
    manifest(&repo, "0.2.0");
    commit(&repo);
    let output = typ(&root, &["update", "example"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("identity changed"));
    let installed = fs::read_to_string(root.join("local/example/0.1.0/typst.toml")).unwrap();
    assert!(installed.contains("0.1.0"));
}

#[test]
fn installs_zip_and_rejects_unsafe_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("packages");
    let zip = temp.path().join("package.zip");
    let mut archive = zip::ZipWriter::new(fs::File::create(&zip).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("example/typst.toml", options).unwrap();
    archive
        .write_all(b"[package]\nname = 'example'\nversion = '1.0.0'\n")
        .unwrap();
    archive.finish().unwrap();
    success(typ(&root, &["install", zip.to_str().unwrap()]));
    assert!(root.join("local/example/1.0.0/typst.toml").is_file());
    success(typ(&root, &["uninstall", "example", "1.0.0"]));

    let mut archive = zip::ZipWriter::new(fs::File::create(&zip).unwrap());
    archive.start_file("../escape", options).unwrap();
    archive.write_all(b"unsafe").unwrap();
    archive.finish().unwrap();
    assert!(!typ(&root, &["install", zip.to_str().unwrap()])
        .status
        .success());
    assert!(!temp.path().join("escape").exists());
}

#[test]
fn installs_tar_gz_and_skips_non_git_update() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("packages");
    let archive = temp.path().join("package.tar.gz");
    let encoder = flate2::write::GzEncoder::new(
        fs::File::create(&archive).unwrap(),
        flate2::Compression::default(),
    );
    let mut tar = tar::Builder::new(encoder);
    let manifest = b"[package]\nname = 'example'\nversion = '1.0.0'\n";
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "typst.toml", &manifest[..])
        .unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    success(typ(&root, &["install", archive.to_str().unwrap()]));
    assert!(success(typ(&root, &["update", "--all"])).contains("skipped"));
}
