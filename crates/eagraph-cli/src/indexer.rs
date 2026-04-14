use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};

use eagraph_core::{FileRecord, GraphStore, RepoConfig};
use eagraph_parser::LanguageExtractor;
use eagraph_store_sqlite::SqliteGraphStore;

use crate::config_loader;

pub struct IndexResult {
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub symbols_count: usize,
    pub edges_count: usize,
}

pub fn index_repo(
    config: &RepoConfig,
    data_dir: &Path,
    org: &str,
    registry: &eagraph_parser::LanguageRegistry,
    force: bool,
) -> Result<IndexResult> {
    let branch = config_loader::detect_branch(&config.root)
        .unwrap_or_else(|_| "main".to_string());
    let db_path = config_loader::db_path(&data_dir.to_path_buf(), org, &config.name, &branch);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let store = SqliteGraphStore::open(&db_path)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let files = collect_files(&config.root, &config.include, &config.exclude)?;

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {bar:30} {pos}/{len} files  {msg}")
            .expect("bad progress template")
            .progress_chars("=> "),
    );

    let mut result = IndexResult {
        files_indexed: 0,
        files_skipped: 0,
        symbols_count: 0,
        edges_count: 0,
    };

    for file_path in &files {
        let rel_path = file_path
            .strip_prefix(&config.root)
            .unwrap_or(file_path);

        pb.set_message(rel_path.display().to_string());

        let indexed = (|| -> Result<bool> {
            let source = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(e) => {
                    pb.suspend(|| eprintln!("  skip {}: {}", rel_path.display(), e));
                    return Ok(false);
                }
            };

            let hash = content_hash(&source);

            if !force {
                if let Ok(Some(existing)) = store.get_file_record(&rel_path.to_path_buf()) {
                    if existing.content_hash == hash {
                        return Ok(false);
                    }
                }
            }

            let extractor = match registry.extractor_for(file_path) {
                Some(ext) => ext,
                None => return Ok(false),
            };
            let (symbols, edges) = match extractor.extract(file_path, &source) {
                Ok(r) => r,
                Err(e) => {
                    pb.suspend(|| eprintln!("  skip {}: {}", rel_path.display(), e));
                    return Ok(false);
                }
            };

            let _ = store.delete_file_data(&rel_path.to_path_buf());

            result.symbols_count += symbols.len();
            result.edges_count += edges.len();

            if !symbols.is_empty() {
                store.upsert_symbols(&symbols)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
            if !edges.is_empty() {
                store.upsert_edges(&edges)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_secs();
            store
                .upsert_file_record(&FileRecord {
                    path: rel_path.to_path_buf(),
                    content_hash: hash,
                    last_indexed: now,
                })
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            Ok(true)
        })();

        match indexed {
            Ok(true) => result.files_indexed += 1,
            Ok(false) => result.files_skipped += 1,
            Err(e) => return Err(e),
        }
        pb.inc(1);
    }

    pb.finish_and_clear();
    Ok(result)
}

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn collect_files(
    root: &Path,
    include: &[String],
    exclude: &[String],
) -> Result<Vec<PathBuf>> {
    // Build a walker that respects .gitignore
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(true)         // skip dotfiles/dotdirs
        .git_ignore(true)     // respect .gitignore
        .git_global(true)     // respect global gitignore
        .git_exclude(true);   // respect .git/info/exclude

    // Add exclude overrides
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for pattern in exclude {
        overrides
            .add(&format!("!{}", pattern))
            .with_context(|| format!("bad exclude pattern: {}", pattern))?;
    }
    // Add include patterns (if specified, only match these)
    if !include.is_empty() {
        for pattern in include {
            overrides
                .add(pattern)
                .with_context(|| format!("bad include pattern: {}", pattern))?;
        }
    }
    builder.overrides(overrides.build()?);

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.with_context(|| "walking directory")?;
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}
