use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

const MARKER: &str = "# typ shell integration";

const POSIX: &str = r#"typ() {
    if [ "${1-}" = cd ]; then
        local dir
        dir=$(command typ --shell-integrated "$@") && builtin cd -- "$dir"
    else
        command typ "$@"
    fi
}"#;

const FISH: &str = r#"function typ
    if test (count $argv) -gt 0; and test "$argv[1]" = cd
        set -l dir (command typ --shell-integrated $argv); and builtin cd -- "$dir"
    else
        command typ $argv
    end
end"#;

const POWERSHELL: &str = r#"function typ {
    $exe = (Get-Command typ -CommandType Application | Select-Object -First 1).Source
    if ($args.Count -gt 0 -and $args[0] -eq 'cd') {
        $dir = & $exe --shell-integrated @args
        if ($LASTEXITCODE -eq 0) { Set-Location -LiteralPath $dir }
    } else {
        & $exe @args
    }
}"#;

fn configure(path: &Path, command: &str) -> Result<()> {
    let existing = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let old = [
        "eval \"$(command typ shell-init zsh)\"",
        "eval \"$(command typ shell-init bash)\"",
        "command typ shell-init fish | source",
        "Invoke-Expression (typ shell-init powershell | Out-String)",
    ];
    for previous in old {
        let block = format!("{MARKER}\n{previous}");
        if existing.contains(&block) {
            fs::write(
                path,
                existing.replacen(&block, &format!("{MARKER}\n{command}"), 1),
            )?;
            return Ok(());
        }
    }
    if !existing.lines().any(|line| line == MARKER) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "\n{MARKER}\n{command}")?;
    }
    Ok(())
}

fn current_shell() -> String {
    let system = sysinfo::System::new_all();
    let parent = system
        .process(sysinfo::Pid::from_u32(std::process::id()))
        .and_then(|process| process.parent())
        .and_then(|pid| system.process(pid))
        .and_then(|process| Path::new(process.name()).file_stem())
        .and_then(|name| name.to_str());
    match parent {
        Some(name @ ("pwsh" | "powershell" | "zsh" | "bash" | "fish")) => name.to_owned(),
        _ => std::env::var("SHELL")
            .ok()
            .and_then(|path| Path::new(&path).file_name()?.to_str().map(str::to_owned))
            .unwrap_or_default(),
    }
}

pub fn ensure_configured() -> Result<()> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let shell = current_shell();
    let (path, command, reload) = match shell.as_str() {
        "zsh" => (home.join(".zshrc"), POSIX, "source ~/.zshrc"),
        "bash" => (home.join(".bashrc"), POSIX, "source ~/.bashrc"),
        "fish" => {
            let config = std::env::var_os("XDG_CONFIG_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            (
                config.join("fish/config.fish"),
                FISH,
                "source ~/.config/fish/config.fish",
            )
        }
        "pwsh" | "powershell" => {
            let config = if cfg!(windows) {
                let documents =
                    dirs::document_dir().context("cannot determine Documents directory")?;
                documents.join(if shell == "pwsh" {
                    "PowerShell"
                } else {
                    "WindowsPowerShell"
                })
            } else {
                std::env::var_os("XDG_CONFIG_HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| home.join(".config"))
                    .join("powershell")
            };
            (
                config.join("Microsoft.PowerShell_profile.ps1"),
                POWERSHELL,
                ". $PROFILE",
            )
        }
        _ => {
            eprintln!("Shell integration is required; see typst-local/README.md.");
            return Ok(());
        }
    };
    configure(&path, command).with_context(|| format!("cannot configure {}", path.display()))?;
    eprintln!(
        "Shell integration configured in {}. Run `{reload}` or open a new terminal, then retry.",
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_old_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".zshrc");
        fs::write(
            &path,
            format!("before\n{MARKER}\neval \"$(command typ shell-init zsh)\"\nafter\n"),
        )
        .unwrap();
        configure(&path, POSIX).unwrap();
        let result = fs::read_to_string(path).unwrap();
        assert!(result.contains(POSIX));
        assert!(!result.contains("shell-init"));
        assert!(result.starts_with("before\n") && result.ends_with("\nafter\n"));
    }

    #[test]
    fn configuration_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".zshrc");
        configure(&path, "eval example").unwrap();
        configure(&path, "eval example").unwrap();
        assert_eq!(fs::read_to_string(path).unwrap().matches(MARKER).count(), 1);
    }
}
