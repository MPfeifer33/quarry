mod audit;
mod cli;
mod manifest;
mod report;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = run(&cli);
    match result {
        Ok(()) => {}
        Err(e) => {
            let code = e.exit_code();
            if cli.is_json() {
                let err_json = serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": e.error_code(),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&err_json).unwrap());
            } else {
                eprintln!("error: {e}");
            }
            std::process::exit(code);
        }
    }
}

fn run(cli: &Cli) -> Result<(), QuarryError> {
    let repo = cli.resolve_repo()?;
    let manifest_data = manifest::parse_manifest(&repo)?;

    match &cli.command {
        Command::Audit { issues_only } => {
            let result = audit::audit(&manifest_data, *issues_only);
            report::print_audit(&manifest_data, &result, cli.is_json())
        }
        Command::Tree => {
            report::print_deps_list(&manifest_data, cli.is_json())
        }
        Command::Pins => {
            report::print_pins(&manifest_data, cli.is_json())
        }
        Command::Plan => {
            // For MVP, plan is a simplified view of what to update
            let result = audit::audit(&manifest_data, false);
            if cli.is_json() {
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "plan": "Update plan based on audit findings",
                    "steps": result.findings.iter()
                        .filter(|f| matches!(f.severity, audit::Severity::Warning | audit::Severity::Error))
                        .map(|f| serde_json::json!({
                            "dep": f.dep,
                            "action": format!("{}", f.message),
                        }))
                        .collect::<Vec<_>>(),
                }))?);
            } else {
                println!("quarry plan: {}", manifest_data.project_name);
                println!();
                let actionable: Vec<_> = result.findings.iter()
                    .filter(|f| matches!(f.severity, audit::Severity::Warning | audit::Severity::Error))
                    .collect();
                if actionable.is_empty() {
                    println!("  No updates needed. Dependencies are clean.");
                } else {
                    println!("  Suggested actions ({}):", actionable.len());
                    for (i, finding) in actionable.iter().enumerate() {
                        println!("  {}. {}", i + 1, finding.message);
                    }
                }
            }
            Ok(())
        }
        Command::Stats => {
            report::print_stats(&manifest_data, cli.is_json())
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QuarryError {
    #[error("{0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl QuarryError {
    pub fn exit_code(&self) -> i32 {
        match self {
            QuarryError::Validation(_) => 1,
            QuarryError::NotFound(_) => 3,
            QuarryError::Io(_) => 2,
            QuarryError::Json(_) => 1,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            QuarryError::Validation(_) => "validation_error",
            QuarryError::NotFound(_) => "not_found",
            QuarryError::Io(_) => "io_error",
            QuarryError::Json(_) => "json_error",
        }
    }
}
