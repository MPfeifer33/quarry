use crate::audit::{AuditResult, Severity};
use crate::manifest::ManifestData;
use crate::QuarryError;

pub fn print_audit(manifest: &ManifestData, result: &AuditResult, is_json: bool) -> Result<(), QuarryError> {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "project": manifest.project_name,
            "project_type": manifest.project_type,
            "version": manifest.project_version,
            "audit": {
                "findings": result.findings,
                "summary": result.summary,
            }
        }))?);
    } else {
        println!("quarry audit: {} ({} {})",
            manifest.project_name, manifest.project_type.label(), manifest.project_version);
        println!();

        if result.findings.is_empty() {
            println!("  No issues found. Dependencies look clean.");
        } else {
            for finding in &result.findings {
                let icon = match finding.severity {
                    Severity::Error => "✗",
                    Severity::Warning => "⚠",
                    Severity::Info => "·",
                };
                println!("  {icon} {}", finding.message);
            }
        }

        println!();
        println!("  Summary: {} deps ({} registry, {} path, {} git, {} dev)",
            result.summary.total_deps,
            result.summary.registry_deps,
            result.summary.path_deps,
            result.summary.git_deps,
            result.summary.dev_deps,
        );
        if result.summary.errors > 0 || result.summary.warnings > 0 {
            println!("  Issues: {} errors, {} warnings",
                result.summary.errors, result.summary.warnings);
        }
    }
    Ok(())
}

pub fn print_deps_list(manifest: &ManifestData, is_json: bool) -> Result<(), QuarryError> {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "project": manifest.project_name,
            "dependencies": manifest.dependencies,
        }))?);
    } else {
        println!("quarry tree: {} ({} {})",
            manifest.project_name, manifest.project_type.label(), manifest.project_version);
        println!();

        let regular: Vec<_> = manifest.dependencies.iter().filter(|d| !d.dev_only).collect();
        let dev: Vec<_> = manifest.dependencies.iter().filter(|d| d.dev_only).collect();

        if !regular.is_empty() {
            println!("  Dependencies ({}):", regular.len());
            for dep in &regular {
                let source_tag = match dep.source {
                    crate::manifest::DependencySource::Registry => "",
                    crate::manifest::DependencySource::Path => " [path]",
                    crate::manifest::DependencySource::Git => " [git]",
                };
                let features_tag = if dep.features.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", dep.features.join(", "))
                };
                println!("    {} {}{}{}", dep.name, dep.version_req, source_tag, features_tag);
            }
        }

        if !dev.is_empty() {
            println!();
            println!("  Dev Dependencies ({}):", dev.len());
            for dep in &dev {
                println!("    {} {}", dep.name, dep.version_req);
            }
        }
    }
    Ok(())
}

pub fn print_pins(manifest: &ManifestData, is_json: bool) -> Result<(), QuarryError> {
    let pinned: Vec<_> = manifest.dependencies.iter()
        .filter(|d| {
            let v = &d.version_req;
            !v.starts_with('^') && !v.starts_with('~')
                && !v.contains(">=") && !v.contains("<=")
                && v != "*"
                && v.split('.').count() == 3
                && v.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .collect();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "pinned_count": pinned.len(),
            "pinned": pinned.iter().map(|d| serde_json::json!({
                "name": d.name,
                "version": d.version_req,
            })).collect::<Vec<_>>(),
        }))?);
    } else {
        if pinned.is_empty() {
            println!("No exact version pins found.");
        } else {
            println!("quarry pins: {} exact pin(s) found", pinned.len());
            println!();
            for dep in &pinned {
                println!("  {} = {}", dep.name, dep.version_req);
            }
            println!();
            println!("  Exact pins prevent automatic compatible updates.");
            println!("  Consider using ^ (caret) ranges for semver flexibility.");
        }
    }
    Ok(())
}

pub fn print_stats(manifest: &ManifestData, is_json: bool) -> Result<(), QuarryError> {
    let total = manifest.dependencies.len();
    let dev = manifest.dependencies.iter().filter(|d| d.dev_only).count();
    let path = manifest.dependencies.iter().filter(|d| d.source == crate::manifest::DependencySource::Path).count();
    let git = manifest.dependencies.iter().filter(|d| d.source == crate::manifest::DependencySource::Git).count();
    let registry = manifest.dependencies.iter().filter(|d| d.source == crate::manifest::DependencySource::Registry).count();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "project": manifest.project_name,
            "stats": {
                "total": total,
                "registry": registry,
                "path": path,
                "git": git,
                "dev": dev,
                "lockfile_exists": manifest.lockfile_exists,
                "lockfile_stale": manifest.lockfile_stale,
            }
        }))?);
    } else {
        println!("quarry stats: {}", manifest.project_name);
        println!();
        println!("  Total dependencies: {total}");
        println!("    Registry: {registry}");
        println!("    Path: {path}");
        println!("    Git: {git}");
        println!("    Dev-only: {dev}");
        println!();
        println!("  Lockfile: {}", if manifest.lockfile_exists {
            if manifest.lockfile_stale { "exists (stale)" } else { "exists (current)" }
        } else {
            "missing"
        });
    }
    Ok(())
}
