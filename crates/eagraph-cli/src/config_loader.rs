use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use eagraph_core::Config;

/// Resolve the config file path. Priority:
/// 1. --config CLI flag
/// 2. EAGRAPH_CONFIG env var
/// 3. OS application data directory
pub fn resolve_config_path(cli_override: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = cli_override {
        return Ok(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("EAGRAPH_CONFIG") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::config_dir().context("could not determine config directory")?;
    Ok(base.join("eagraph").join("config.toml"))
}

/// Resolve the data directory for storing DBs.
pub fn resolve_data_dir(cli_override: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = cli_override {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::data_dir()
        .or_else(dirs::config_dir)
        .context("could not determine data directory")?;
    Ok(base.join("eagraph").join("data"))
}

/// Resolve the grammars directory for tree-sitter grammar .so + .scm + .toml files.
pub fn resolve_grammars_dir(cli_override: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = cli_override {
        return Ok(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("EAGRAPH_GRAMMARS") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::config_dir().context("could not determine config directory")?;
    Ok(base.join("eagraph").join("grammars"))
}

pub fn load_config(path: &Path) -> Result<Config> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let config: Config =
        toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    Ok(config)
}

/// Build the DB path for a given org/repo/branch.
pub fn db_path(data_dir: &Path, org: &str, repo: &str, branch: &str) -> PathBuf {
    let sanitized_branch = branch.replace('/', "--");
    data_dir
        .join(org)
        .join(repo)
        .join(format!("{}.db", sanitized_branch))
}

/// Detect the current branch for a repo and build the DB path for it.
/// Returns `(branch, db_path)`. Propagates branch-detection errors.
pub fn resolve_db_path(
    data_dir: &Path,
    org: &str,
    repo_name: &str,
    repo_root: &Path,
) -> Result<(String, PathBuf)> {
    let branch = detect_branch(repo_root)?;
    let path = db_path(data_dir, org, repo_name, &branch);
    Ok((branch, path))
}

/// Detect the current git branch for a repo root.
pub fn detect_branch(repo_root: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_root)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed in {}: {}",
            repo_root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
