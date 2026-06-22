use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::QuarryError;

#[derive(Parser, Debug)]
#[command(name = "quarry", version, about = "Dependency audit and update planner")]
pub struct Cli {
    /// Project root override
    #[arg(long, global = true)]
    pub repo: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn resolve_repo(&self) -> Result<PathBuf, QuarryError> {
        if let Some(ref repo) = self.repo {
            return Ok(repo.clone());
        }
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(PathBuf::from(path));
            }
        }
        std::env::current_dir().map_err(QuarryError::Io)
    }

    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Audit dependencies: list all with versions, flag outdated/pinned
    Audit {
        /// Show only dependencies with issues
        #[arg(long)]
        issues_only: bool,
    },
    /// Show dependency tree (direct + what they pull in)
    Tree,
    /// Check for version pinning issues
    Pins,
    /// Suggest an update plan (safe order based on semver)
    Plan,
    /// Show summary statistics
    Stats,
}
