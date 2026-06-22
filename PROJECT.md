# PROJECT.md — quarry

**What:** Dependency audit and update planner. Parses project manifests, flags version issues, stale lockfiles, and non-registry sources.

**Status:** MVP complete. Audit, tree, pins, plan, stats all working. Supports Rust, Node, Python, Go.

**Tech:** Rust 2021, clap 4, serde/serde_json, toml, regex, thiserror.

## Module Ownership

| Module | Owner | Status |
|--------|-------|--------|
| cli.rs | Nix | Done |
| main.rs | Nix | Done |
| manifest.rs | Nix | Done |
| audit.rs | Nix | Done |
| report.rs | Nix | Done (Bjarn enhancing) |

## Usage

```sh
quarry audit                        # check deps for issues
quarry audit --issues-only          # only show problems
quarry tree                         # list all dependencies
quarry pins                         # show exact version pins
quarry plan                         # suggest update actions
quarry stats                        # dependency summary
```

## Check Categories

| Issue | Severity | Description |
|-------|----------|-------------|
| wildcard_version | warning | `*` version spec |
| exact_pin | info | Pinned to exact version, no range |
| git_source | warning | Dep sourced from git, not registry |
| path_source | info | Local path dependency |
| stale_lockfile | warning | Lockfile older than manifest |
| no_lockfile | warning | No lockfile found |
| major_zero | info | Pre-1.0 dependency |

## Last Updated

2026-06-22 — Initial skeleton with audit/tree/pins/plan/stats working.
