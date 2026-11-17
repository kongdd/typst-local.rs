mod archive;
mod git;
mod install;
mod package;
mod shell;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Manage Typst local packages")]
struct Cli {
    #[arg(long, hide = true, global = true)]
    shell_integrated: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install from a Git repository, directory, or archive
    Install { source: String },
    /// Update a package or all packages from Git
    Update {
        #[arg(required_unless_present = "all")]
        name: Option<String>,
        #[arg(long, conflicts_with = "name")]
        all: bool,
    },
    /// List installed local packages
    List,
    /// Uninstall a specific package version
    Uninstall { name: String, version: String },
    /// Print a package directory (requires shell integration to change directory)
    Cd { name: String },
    /// Print the local package directory
    Path,
}

fn main() -> Result<()> {
    let root = package::root()?;
    let cli = Cli::parse();
    match cli.command {
        Command::Install { source } => install::from_source(&root, &source),
        Command::Update { name, all } => match name {
            Some(name) => git::update(&root, &name),
            None if all => git::update_all(&root),
            None => unreachable!(),
        },
        Command::List => package::list(&root),
        Command::Uninstall { name, version } => package::uninstall(&root, &name, &version),
        Command::Cd { name } => {
            let path = package::directory(&root, &name)?;
            if !cli.shell_integrated {
                shell::ensure_configured()?;
            }
            println!("{}", path.display());
            Ok(())
        }
        Command::Path => {
            println!("{}", root.display());
            Ok(())
        }
    }
}
