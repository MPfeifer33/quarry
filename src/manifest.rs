use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::QuarryError;

/// A parsed dependency from any manifest type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version_req: String,
    pub source: DependencySource,
    pub features: Vec<String>,
    pub optional: bool,
    pub dev_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencySource {
    Registry,
    Path,
    Git,
}

/// Detected project type and its dependencies.
#[derive(Debug, Serialize)]
pub struct ManifestData {
    pub project_type: ProjectType,
    pub project_name: String,
    pub project_version: String,
    pub dependencies: Vec<Dependency>,
    pub lockfile_exists: bool,
    pub lockfile_stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Unknown used as fallback in detection
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Unknown,
}

impl ProjectType {
    pub fn label(&self) -> &'static str {
        match self {
            ProjectType::Rust => "rust",
            ProjectType::Node => "node",
            ProjectType::Python => "python",
            ProjectType::Go => "go",
            ProjectType::Unknown => "unknown",
        }
    }
}

/// Detect and parse the project manifest.
pub fn parse_manifest(repo: &Path) -> Result<ManifestData, QuarryError> {
    if repo.join("Cargo.toml").exists() {
        parse_cargo(repo)
    } else if repo.join("package.json").exists() {
        parse_npm(repo)
    } else if repo.join("pyproject.toml").exists() {
        parse_pyproject(repo)
    } else if repo.join("go.mod").exists() {
        parse_gomod(repo)
    } else {
        Err(QuarryError::Validation("No recognized manifest found (Cargo.toml, package.json, pyproject.toml, go.mod)".into()))
    }
}

fn parse_cargo(repo: &Path) -> Result<ManifestData, QuarryError> {
    let content = std::fs::read_to_string(repo.join("Cargo.toml"))?;
    let parsed: toml::Value = content.parse()
        .map_err(|e: toml::de::Error| QuarryError::Validation(format!("Invalid Cargo.toml: {e}")))?;

    let package = parsed.get("package").and_then(|p| p.as_table());
    let project_name = package
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let project_version = package
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    let mut dependencies = Vec::new();

    // Regular deps
    if let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_table()) {
        for (name, value) in deps {
            dependencies.push(parse_cargo_dep(name, value, false));
        }
    }

    // Dev deps
    if let Some(deps) = parsed.get("dev-dependencies").and_then(|d| d.as_table()) {
        for (name, value) in deps {
            dependencies.push(parse_cargo_dep(name, value, true));
        }
    }

    // Build deps
    if let Some(deps) = parsed.get("build-dependencies").and_then(|d| d.as_table()) {
        for (name, value) in deps {
            dependencies.push(parse_cargo_dep(name, value, false));
        }
    }

    let lockfile = repo.join("Cargo.lock");
    let lockfile_exists = lockfile.exists();
    let lockfile_stale = is_lockfile_stale(repo, "Cargo.toml", "Cargo.lock");

    Ok(ManifestData {
        project_type: ProjectType::Rust,
        project_name,
        project_version,
        dependencies,
        lockfile_exists,
        lockfile_stale,
    })
}

fn parse_cargo_dep(name: &str, value: &toml::Value, dev_only: bool) -> Dependency {
    let (version_req, source, features, optional) = match value {
        toml::Value::String(v) => (v.clone(), DependencySource::Registry, vec![], false),
        toml::Value::Table(t) => {
            let version = t.get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("*")
                .to_string();
            let source = if t.contains_key("path") {
                DependencySource::Path
            } else if t.contains_key("git") {
                DependencySource::Git
            } else {
                DependencySource::Registry
            };
            let features = t.get("features")
                .and_then(|f| f.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let optional = t.get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            (version, source, features, optional)
        }
        _ => ("*".to_string(), DependencySource::Registry, vec![], false),
    };

    Dependency {
        name: name.to_string(),
        version_req,
        source,
        features,
        optional,
        dev_only,
    }
}

fn parse_npm(repo: &Path) -> Result<ManifestData, QuarryError> {
    let content = std::fs::read_to_string(repo.join("package.json"))?;
    let parsed: serde_json::Value = serde_json::from_str(&content)?;

    let project_name = parsed["name"].as_str().unwrap_or("unknown").to_string();
    let project_version = parsed["version"].as_str().unwrap_or("0.0.0").to_string();

    let mut dependencies = Vec::new();

    if let Some(deps) = parsed["dependencies"].as_object() {
        for (name, version) in deps {
            dependencies.push(Dependency {
                name: name.clone(),
                version_req: version.as_str().unwrap_or("*").to_string(),
                source: DependencySource::Registry,
                features: vec![],
                optional: false,
                dev_only: false,
            });
        }
    }

    if let Some(deps) = parsed["devDependencies"].as_object() {
        for (name, version) in deps {
            dependencies.push(Dependency {
                name: name.clone(),
                version_req: version.as_str().unwrap_or("*").to_string(),
                source: DependencySource::Registry,
                features: vec![],
                optional: false,
                dev_only: true,
            });
        }
    }

    let lockfile_exists = repo.join("package-lock.json").exists()
        || repo.join("yarn.lock").exists()
        || repo.join("pnpm-lock.yaml").exists();

    let lockfile_stale = if repo.join("package-lock.json").exists() {
        is_lockfile_stale(repo, "package.json", "package-lock.json")
    } else {
        false
    };

    Ok(ManifestData {
        project_type: ProjectType::Node,
        project_name,
        project_version,
        dependencies,
        lockfile_exists,
        lockfile_stale,
    })
}

fn parse_pyproject(repo: &Path) -> Result<ManifestData, QuarryError> {
    let content = std::fs::read_to_string(repo.join("pyproject.toml"))?;
    let parsed: toml::Value = content.parse()
        .map_err(|e: toml::de::Error| QuarryError::Validation(format!("Invalid pyproject.toml: {e}")))?;

    let project = parsed.get("project").and_then(|p| p.as_table());
    let project_name = project
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let project_version = project
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    let mut dependencies = Vec::new();

    if let Some(deps) = project.and_then(|p| p.get("dependencies")).and_then(|d| d.as_array()) {
        for dep in deps {
            if let Some(dep_str) = dep.as_str() {
                let (name, version) = parse_pep508(dep_str);
                dependencies.push(Dependency {
                    name,
                    version_req: version,
                    source: DependencySource::Registry,
                    features: vec![],
                    optional: false,
                    dev_only: false,
                });
            }
        }
    }

    Ok(ManifestData {
        project_type: ProjectType::Python,
        project_name,
        project_version,
        dependencies,
        lockfile_exists: repo.join("poetry.lock").exists() || repo.join("uv.lock").exists(),
        lockfile_stale: false,
    })
}

fn parse_pep508(spec: &str) -> (String, String) {
    let re = regex::Regex::new(r"^([a-zA-Z0-9_-]+)\s*(.*)$").unwrap();
    if let Some(cap) = re.captures(spec.trim()) {
        let name = cap[1].to_string();
        let version = cap.get(2).map_or("*", |m| m.as_str()).trim().to_string();
        (name, if version.is_empty() { "*".to_string() } else { version })
    } else {
        (spec.to_string(), "*".to_string())
    }
}

fn parse_gomod(repo: &Path) -> Result<ManifestData, QuarryError> {
    let content = std::fs::read_to_string(repo.join("go.mod"))?;

    let module_re = regex::Regex::new(r"module\s+(.+)").unwrap();
    let project_name = module_re.captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let go_re = regex::Regex::new(r"go\s+([\d.]+)").unwrap();
    let project_version = go_re.captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "0.0.0".to_string());

    let mut dependencies = Vec::new();
    let require_re = regex::Regex::new(r"(?m)^\s+(\S+)\s+(v[\d.]+(?:-\S+)?)").unwrap();

    for cap in require_re.captures_iter(&content) {
        dependencies.push(Dependency {
            name: cap[1].to_string(),
            version_req: cap[2].to_string(),
            source: DependencySource::Registry,
            features: vec![],
            optional: false,
            dev_only: false,
        });
    }

    Ok(ManifestData {
        project_type: ProjectType::Go,
        project_name,
        project_version,
        dependencies,
        lockfile_exists: repo.join("go.sum").exists(),
        lockfile_stale: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_cargo_manifest() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), r#"
[package]
name = "test-proj"
version = "0.1.0"

[dependencies]
serde = "1"
clap = { version = "4", features = ["derive"] }

[dev-dependencies]
tempfile = "3"
"#).unwrap();
        let data = parse_manifest(tmp.path()).unwrap();
        assert_eq!(data.project_type, ProjectType::Rust);
        assert_eq!(data.project_name, "test-proj");
        assert_eq!(data.dependencies.len(), 3);
        assert!(data.dependencies.iter().any(|d| d.name == "serde"));
        assert!(data.dependencies.iter().any(|d| d.dev_only && d.name == "tempfile"));
    }

    #[test]
    fn parse_npm_manifest() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("package.json"), r#"{
            "name": "test-app",
            "version": "1.0.0",
            "dependencies": { "express": "^4.18.0" },
            "devDependencies": { "jest": "^29.0.0" }
        }"#).unwrap();
        let data = parse_manifest(tmp.path()).unwrap();
        assert_eq!(data.project_type, ProjectType::Node);
        assert_eq!(data.dependencies.len(), 2);
    }

    #[test]
    fn parse_pep508_simple() {
        let (name, ver) = parse_pep508("flask>=2.0");
        assert_eq!(name, "flask");
        assert_eq!(ver, ">=2.0");
    }

    #[test]
    fn parse_pep508_no_version() {
        let (name, ver) = parse_pep508("requests");
        assert_eq!(name, "requests");
        assert_eq!(ver, "*");
    }

    #[test]
    fn no_manifest_returns_error() {
        let tmp = TempDir::new().unwrap();
        assert!(parse_manifest(tmp.path()).is_err());
    }

    #[test]
    fn cargo_dep_with_path_source() {
        let v: toml::Value = toml::from_str(r#"path = "../lib""#).unwrap();
        let dep = parse_cargo_dep("mylib", &v, false);
        assert_eq!(dep.source, DependencySource::Path);
    }

    #[test]
    fn cargo_dep_with_git_source() {
        let v: toml::Value = toml::from_str(r#"git = "https://github.com/foo/bar""#).unwrap();
        let dep = parse_cargo_dep("mylib", &v, false);
        assert_eq!(dep.source, DependencySource::Git);
    }
}

fn is_lockfile_stale(repo: &Path, manifest: &str, lockfile: &str) -> bool {
    let manifest_path = repo.join(manifest);
    let lockfile_path = repo.join(lockfile);

    if let (Ok(m_meta), Ok(l_meta)) = (manifest_path.metadata(), lockfile_path.metadata()) {
        if let (Ok(m_time), Ok(l_time)) = (m_meta.modified(), l_meta.modified()) {
            return m_time > l_time;
        }
    }
    false
}
