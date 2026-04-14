use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use eagraph_core::{Edge, FileRecord, GraphStore, RepoConfig, Symbol};
use eagraph_parser::LanguageExtractor;
use eagraph_store_sqlite::SqliteGraphStore;

use crate::config_loader;

pub struct IndexResult {
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub symbols_count: usize,
    pub edges_count: usize,
}

/// Result of parsing a single file (produced in parallel, consumed sequentially).
struct ParsedFile {
    rel_path: PathBuf,
    content_hash: String,
    symbols: Vec<Symbol>,
    edges: Vec<Edge>,
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

    let skipped = AtomicUsize::new(0);

    // Phase 1: read, hash, check, parse — in parallel
    let parsed: Vec<ParsedFile> = files
        .par_iter()
        .filter_map(|file_path| {
            let rel_path = file_path
                .strip_prefix(&config.root)
                .unwrap_or(file_path)
                .to_path_buf();

            pb.set_message(rel_path.display().to_string());

            let result = parse_one(file_path, &rel_path, &store, registry, force, &pb);
            pb.inc(1);

            match result {
                Some(parsed) => Some(parsed),
                None => {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
        })
        .collect();

    pb.finish_and_clear();

    // Phase 2: write all results in a single transaction
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs();

    let mut total_symbols = 0usize;
    let mut total_edges = 0usize;

    store.begin_transaction().map_err(|e| anyhow::anyhow!("{}", e))?;

    for p in &parsed {
        let _ = store.delete_file_data(&p.rel_path);

        if !p.symbols.is_empty() {
            store
                .upsert_symbols(&p.symbols)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        if !p.edges.is_empty() {
            store
                .upsert_edges(&p.edges)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        store
            .upsert_file_record(&FileRecord {
                path: p.rel_path.clone(),
                content_hash: p.content_hash.clone(),
                last_indexed: now,
            })
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        total_symbols += p.symbols.len();
        total_edges += p.edges.len();
    }

    store.commit_transaction().map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok(IndexResult {
        files_indexed: parsed.len(),
        files_skipped: skipped.load(Ordering::Relaxed),
        symbols_count: total_symbols,
        edges_count: total_edges,
    })
}

fn parse_one(
    file_path: &Path,
    rel_path: &Path,
    store: &SqliteGraphStore,
    registry: &eagraph_parser::LanguageRegistry,
    force: bool,
    pb: &ProgressBar,
) -> Option<ParsedFile> {
    let source = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            pb.suspend(|| eprintln!("  skip {}: {}", rel_path.display(), e));
            return None;
        }
    };

    let hash = content_hash(&source);

    if !force {
        if let Ok(Some(existing)) = store.get_file_record(&rel_path.to_path_buf()) {
            if existing.content_hash == hash {
                return None;
            }
        }
    }

    let extractor = registry.extractor_for(file_path)?;

    let (symbols, edges) = match extractor.extract(file_path, &source) {
        Ok(r) => r,
        Err(e) => {
            pb.suspend(|| eprintln!("  skip {}: {}", rel_path.display(), e));
            return None;
        }
    };

    Some(ParsedFile {
        rel_path: rel_path.to_path_buf(),
        content_hash: hash,
        symbols,
        edges,
    })
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
    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);

    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for pattern in exclude {
        overrides
            .add(&format!("!{}", pattern))
            .with_context(|| format!("bad exclude pattern: {}", pattern))?;
    }
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
