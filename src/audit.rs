use serde::Serialize;

use crate::manifest::{Dependency, DependencySource, ManifestData};

/// Issues found during audit.
#[derive(Debug, Serialize)]
pub struct AuditResult {
    pub findings: Vec<AuditFinding>,
    pub summary: AuditSummary,
}

#[derive(Debug, Serialize)]
pub struct AuditFinding {
    pub dep: String,
    pub issue: IssueKind,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    WildcardVersion,
    ExactPin,
    GitSource,
    PathSource,
    StaleLockfile,
    NoLockfile,
    MajorZero,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Serialize)]
pub struct AuditSummary {
    pub total_deps: usize,
    pub registry_deps: usize,
    pub path_deps: usize,
    pub git_deps: usize,
    pub dev_deps: usize,
    pub issues: usize,
    pub errors: usize,
    pub warnings: usize,
}

pub fn audit(manifest: &ManifestData, issues_only: bool) -> AuditResult {
    let mut findings = Vec::new();

    // Lockfile checks
    if !manifest.lockfile_exists {
        findings.push(AuditFinding {
            dep: "(project)".into(),
            issue: IssueKind::NoLockfile,
            severity: Severity::Warning,
            message: "No lockfile found. Builds may not be reproducible.".into(),
        });
    } else if manifest.lockfile_stale {
        findings.push(AuditFinding {
            dep: "(project)".into(),
            issue: IssueKind::StaleLockfile,
            severity: Severity::Warning,
            message: "Lockfile is older than manifest. Run the package manager to update.".into(),
        });
    }

    // Per-dep checks
    for dep in &manifest.dependencies {
        findings.extend(check_dependency(dep));
    }

    let errors = findings.iter().filter(|f| matches!(f.severity, Severity::Error)).count();
    let warnings = findings.iter().filter(|f| matches!(f.severity, Severity::Warning)).count();

    let summary = AuditSummary {
        total_deps: manifest.dependencies.len(),
        registry_deps: manifest.dependencies.iter().filter(|d| d.source == DependencySource::Registry).count(),
        path_deps: manifest.dependencies.iter().filter(|d| d.source == DependencySource::Path).count(),
        git_deps: manifest.dependencies.iter().filter(|d| d.source == DependencySource::Git).count(),
        dev_deps: manifest.dependencies.iter().filter(|d| d.dev_only).count(),
        issues: findings.len(),
        errors,
        warnings,
    };

    if issues_only {
        // Keep only findings with issues (not info)
        findings.retain(|f| !matches!(f.severity, Severity::Info));
    }

    AuditResult { findings, summary }
}

fn check_dependency(dep: &Dependency) -> Vec<AuditFinding> {
    let mut findings = Vec::new();

    // Wildcard version
    if dep.version_req == "*" && dep.source == DependencySource::Registry {
        findings.push(AuditFinding {
            dep: dep.name.clone(),
            issue: IssueKind::WildcardVersion,
            severity: Severity::Warning,
            message: format!("`{}` has wildcard version. Pin to a specific range.", dep.name),
        });
    }

    // Exact pin (no ^ or ~ or range)
    if is_exact_pin(&dep.version_req) && dep.source == DependencySource::Registry {
        findings.push(AuditFinding {
            dep: dep.name.clone(),
            issue: IssueKind::ExactPin,
            severity: Severity::Info,
            message: format!("`{}` is pinned to exact version {}. May miss compatible updates.", dep.name, dep.version_req),
        });
    }

    // Git source
    if dep.source == DependencySource::Git {
        findings.push(AuditFinding {
            dep: dep.name.clone(),
            issue: IssueKind::GitSource,
            severity: Severity::Warning,
            message: format!("`{}` is sourced from git. Not reproducible without a rev pin.", dep.name),
        });
    }

    // Path source (not necessarily bad, just notable)
    if dep.source == DependencySource::Path {
        findings.push(AuditFinding {
            dep: dep.name.clone(),
            issue: IssueKind::PathSource,
            severity: Severity::Info,
            message: format!("`{}` is a local path dependency.", dep.name),
        });
    }

    // Major version 0.x (pre-1.0 semver)
    if is_major_zero(&dep.version_req) && dep.source == DependencySource::Registry && !dep.dev_only {
        findings.push(AuditFinding {
            dep: dep.name.clone(),
            issue: IssueKind::MajorZero,
            severity: Severity::Info,
            message: format!("`{}` is pre-1.0 ({}). API may change between minor versions.", dep.name, dep.version_req),
        });
    }

    findings
}

fn is_exact_pin(version: &str) -> bool {
    let cleaned = version.trim_start_matches('=').trim();
    // An exact pin is a version like "1.2.3" with no range operators
    if cleaned == "*" || cleaned.is_empty() {
        return false;
    }
    // Has range operator? Not exact.
    if cleaned.starts_with('^') || cleaned.starts_with('~')
        || cleaned.contains(">=") || cleaned.contains("<=")
        || cleaned.contains('>') || cleaned.contains('<')
        || cleaned.contains(',')
    {
        return false;
    }
    // Must have 3 components to be exact
    cleaned.split('.').count() == 3 && cleaned.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn is_major_zero(version: &str) -> bool {
    let cleaned = version
        .trim_start_matches('^')
        .trim_start_matches('~')
        .trim_start_matches(">=")
        .trim_start_matches('=')
        .trim();
    cleaned.starts_with("0.")
}
