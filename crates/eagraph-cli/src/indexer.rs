use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use eagraph_core::{FileRecord, GraphStore, RawEdge, RepoConfig, Symbol};
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
    raw_edges: Vec<RawEdge>,
}

pub fn index_repo(
    config: &RepoConfig,
    data_dir: &Path,
    org: &str,
    registry: &eagraph_parser::LanguageRegistry,
    force: bool,
) -> Result<IndexResult> {
    let (_branch, db_path) =
        config_loader::resolve_db_path(data_dir, org, &config.name, &config.root)?;

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    if force && db_path.exists() {
        std::fs::remove_file(&db_path)
            .with_context(|| format!("removing {}", db_path.display()))?;
    }

    let store = SqliteGraphStore::open(&db_path)?;

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

    store.begin_transaction()?;

    // Pass 1: delete old data + insert all symbols and file records
    for p in &parsed {
        store.delete_file_data(&p.rel_path)?;

        if !p.symbols.is_empty() {
            store.upsert_symbols(&p.symbols)?;
        }
        store.upsert_file_record(&FileRecord {
            path: p.rel_path.clone(),
            content_hash: p.content_hash.clone(),
            last_indexed: now,
        })?;

        total_symbols += p.symbols.len();
    }

    // Pass 2: resolve raw edges → real edges
    let all_symbols: Vec<Symbol> = parsed.iter().flat_map(|p| p.symbols.clone()).collect();
    let all_raw: Vec<RawEdge> = parsed.iter().flat_map(|p| p.raw_edges.clone()).collect();
    let ext_to_lang = registry.ext_to_lang();
    let all_resolved = RawEdge::resolve(&all_raw, &all_symbols, &ext_to_lang);
    let total_edges = all_resolved.len();

    if !all_resolved.is_empty() {
        store.upsert_edges(&all_resolved)?;
    }

    store.commit_transaction()?;

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
        match store.get_file_record(rel_path) {
            Ok(Some(existing)) if existing.content_hash == hash => return None,
            Ok(_) => {}
            Err(e) => {
                pb.suspend(|| {
                    eprintln!(
                        "  skip {}: failed to read file record: {}",
                        rel_path.display(),
                        e
                    )
                });
                return None;
            }
        }
    }

    let extractor = registry.extractor_for(file_path)?;

    let (symbols, raw_edges) = match extractor.extract(file_path, &source) {
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
        raw_edges,
    })
}

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn collect_files(root: &Path, include: &[String], exclude: &[String]) -> Result<Vec<PathBuf>> {
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
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            files.push(entry.into_path());
        }
    }
    files.sort();
    Ok(files)
}

/// Check for stale files and re-index them before a query.
/// Uses mtime comparison — if a file's mtime is newer than its last_indexed
/// timestamp, or if the file is new (not in DB), re-index it.
pub fn auto_refresh(
    store: &SqliteGraphStore,
    repo_config: &RepoConfig,
    registry: &eagraph_parser::LanguageRegistry,
) -> Result<()> {
    let all_files = collect_files(
        &repo_config.root,
        &repo_config.include,
        &repo_config.exclude,
    )?;

    // Get existing file records from DB
    let existing: std::collections::HashMap<PathBuf, u64> = store
        .get_all_file_records()?
        .into_iter()
        .map(|r| (r.path, r.last_indexed))
        .collect();

    // Find stale or new files by checking mtime
    let mut stale_files = Vec::new();
    for file_path in &all_files {
        let rel_path = file_path
            .strip_prefix(&repo_config.root)
            .unwrap_or(file_path);

        // Check if we have a grammar for this file
        if registry.extractor_for(file_path).is_none() {
            continue;
        }

        let mtime = match std::fs::metadata(file_path) {
            Ok(m) => m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            Err(_) => continue,
        };

        match existing.get(rel_path) {
            Some(last_indexed) if mtime <= *last_indexed => {} // up to date
            _ => stale_files.push(file_path.clone()),          // stale or new
        }
    }

    // Check for deleted files (in DB but not on disk)
    let disk_files: std::collections::HashSet<&Path> =
        all_files.iter().map(|p| p.as_path()).collect();
    for db_path in existing.keys() {
        let abs = repo_config.root.join(db_path);
        if !disk_files.contains(abs.as_path()) {
            store.delete_file_data(db_path)?;
        }
    }

    if stale_files.is_empty() {
        return Ok(());
    }

    let count = stale_files.len();

    // Parse stale files. Log and skip files that fail to read or parse.
    let parsed: Vec<ParsedFile> = stale_files
        .iter()
        .filter_map(|file_path| {
            let rel_path = file_path
                .strip_prefix(&repo_config.root)
                .unwrap_or(file_path);
            let source = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("  [auto] skip {}: {}", rel_path.display(), e);
                    return None;
                }
            };
            let hash = content_hash(&source);
            let extractor = registry.extractor_for(file_path)?;
            let (symbols, raw_edges) = match extractor.extract(file_path, &source) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("  [auto] skip {}: {}", rel_path.display(), e);
                    return None;
                }
            };
            Some(ParsedFile {
                rel_path: rel_path.to_path_buf(),
                content_hash: hash,
                symbols,
                raw_edges,
            })
        })
        .collect();

    if parsed.is_empty() {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs();

    store.begin_transaction()?;

    // Delete old data + insert symbols
    for p in &parsed {
        store.delete_file_data(&p.rel_path)?;
        if !p.symbols.is_empty() {
            store.upsert_symbols(&p.symbols)?;
        }
        store.upsert_file_record(&FileRecord {
            path: p.rel_path.clone(),
            content_hash: p.content_hash.clone(),
            last_indexed: now,
        })?;
    }

    // Resolve edges against full symbol table
    let all_symbols = store.get_all_symbols()?;
    let all_raw: Vec<eagraph_core::RawEdge> =
        parsed.iter().flat_map(|p| p.raw_edges.clone()).collect();
    let ext_to_lang = registry.ext_to_lang();
    let resolved = eagraph_core::RawEdge::resolve(&all_raw, &all_symbols, &ext_to_lang);

    if !resolved.is_empty() {
        store.upsert_edges(&resolved)?;
    }

    store.commit_transaction()?;

    let sym_count: usize = parsed.iter().map(|p| p.symbols.len()).sum();
    eprintln!(
        "  [auto] {} files refreshed, {} symbols, {} edges",
        count,
        sym_count,
        resolved.len()
    );

    Ok(())
}
