mod config_loader;
mod grammar_builder;
mod indexer;
mod viz;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use eagraph_core::GraphStore;
use eagraph_store_sqlite::SqliteGraphStore;

#[derive(Parser)]
#[command(name = "eagraph", about = "Multi-repo code knowledge graph")]
struct Cli {
    /// Path to config.toml (overrides EAGRAPH_CONFIG and default location)
    #[arg(long, global = true)]
    config: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new config file
    Init {
        /// Organization name
        org: String,
    },
    /// Add a repo to the config
    Add {
        /// Path to the repo root
        path: String,
        /// Override repo name (defaults to folder name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Index a repo (or all with --all)
    Index {
        /// Repo name to index
        repo: Option<String>,
        /// Index all configured repos
        #[arg(long)]
        all: bool,
        /// Re-index all files, ignoring content hashes
        #[arg(long)]
        force: bool,
    },
    /// Show all repos, branches, symbol/edge counts
    Status,
    /// Search for symbols by name
    Query {
        /// Symbol name to search for
        name: String,
        /// Limit search to this repo
        #[arg(long)]
        repo: Option<String>,
    },
    /// Get structural context for a symbol (neighbors + source snippets)
    Context {
        /// Symbol name
        name: String,
        /// Repo name (auto-detected from cwd if omitted)
        #[arg(long)]
        repo: Option<String>,
        /// Traversal depth
        #[arg(long, default_value = "2")]
        depth: u32,
    },
    /// Show what depends on symbols in a file
    Dependents {
        /// File path (relative to repo root)
        file: String,
        /// Repo name (auto-detected from cwd if omitted)
        #[arg(long)]
        repo: Option<String>,
        /// Traversal depth
        #[arg(long, default_value = "1")]
        depth: u32,
    },
    /// List all symbols in a file
    Symbols {
        /// File path (relative to repo root)
        file: String,
        /// Repo name (auto-detected from cwd if omitted)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Find shortest call path between two symbols
    Chain {
        /// Source symbol name
        from: String,
        /// Target symbol name
        to: String,
        /// Repo name (auto-detected from cwd if omitted)
        #[arg(long)]
        repo: Option<String>,
    },
    /// Open interactive graph visualization in browser
    Viz {
        /// Port to serve on
        #[arg(long, default_value = "3742")]
        port: u16,
    },
    /// Print resolved config path and contents
    Config,
    /// Manage tree-sitter grammars
    Grammars {
        #[command(subcommand)]
        action: GrammarsAction,
    },
}

#[derive(Subcommand)]
enum GrammarsAction {
    /// Download, compile, and install a grammar
    Add {
        /// Language names (e.g. python typescript rust)
        names: Vec<String>,
    },
    /// Show available and installed grammars
    List,
    /// Scan repos and recommend grammars to install
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let grammars_dir = config_loader::resolve_grammars_dir(None)?;

    // These grammars subcommands don't need config.toml
    if let Command::Grammars { action } = &cli.command {
        match action {
            GrammarsAction::Add { names } => {
                return grammar_builder::cmd_grammars_add(names, &grammars_dir)
            }
            GrammarsAction::List => return grammar_builder::cmd_grammars_list(&grammars_dir),
            GrammarsAction::Check => {} // needs config, fall through
        }
    }

    let config_path = config_loader::resolve_config_path(cli.config.as_deref())?;

    // Commands that don't need a loaded config
    match &cli.command {
        Command::Config => return cmd_config(&config_path, &grammars_dir),
        Command::Init { org } => return cmd_init(&config_path, org),
        Command::Add { path, name } => {
            return cmd_add(&config_path, path, name.as_deref(), &grammars_dir)
        }
        _ => {}
    }

    if !config_path.exists() {
        eprintln!("Config file not found. Run:");
        eprintln!();
        eprintln!("  eagraph init <org-name>");
        std::process::exit(1);
    }
    let config = config_loader::load_config(&config_path)?;
    let data_dir = config_loader::resolve_data_dir(None)?;
    let registry = eagraph_parser::LanguageRegistry::from_dir(&grammars_dir)?;

    let json = cli.json;
    match cli.command {
        Command::Index { repo, all, force } => {
            cmd_index(&config, &data_dir, &registry, repo.as_deref(), all, force)
        }
        Command::Status => cmd_status(&config, &data_dir, &registry, json),
        Command::Query { name, repo } => {
            cmd_query(&config, &data_dir, &registry, &name, repo.as_deref(), json)
        }
        Command::Context { name, repo, depth } => {
            let r = resolve_repo_name(&config, repo.as_deref())?;
            cmd_context(&config, &data_dir, &registry, &name, &r, depth, json)
        }
        Command::Dependents { file, repo, depth } => {
            let r = resolve_repo_name(&config, repo.as_deref())?;
            cmd_dependents(&config, &data_dir, &registry, &file, &r, depth, json)
        }
        Command::Symbols { file, repo } => {
            let r = resolve_repo_name(&config, repo.as_deref())?;
            cmd_symbols(&config, &data_dir, &registry, &file, &r, json)
        }
        Command::Chain { from, to, repo } => {
            let r = resolve_repo_name(&config, repo.as_deref())?;
            cmd_chain(&config, &data_dir, &registry, &from, &to, &r, json)
        }
        Command::Viz { port } => cmd_viz(&config, &data_dir, &registry, port),
        Command::Grammars {
            action: GrammarsAction::Check,
        } => grammar_builder::cmd_grammars_check(&config, &grammars_dir),
        Command::Config | Command::Init { .. } | Command::Add { .. } | Command::Grammars { .. } => {
            unreachable!()
        }
    }
}

fn cmd_index(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    repo_filter: Option<&str>,
    all: bool,
    force: bool,
) -> Result<()> {
    if repo_filter.is_none() && !all {
        eprintln!("Specify a repo name or use --all:");
        eprintln!();
        eprintln!("  eagraph index <repo-name>");
        eprintln!("  eagraph index --all");
        eprintln!();
        if !config.repos.is_empty() {
            eprintln!("Configured repos:");
            for r in &config.repos {
                eprintln!("  {}", r.name);
            }
        }
        std::process::exit(1);
    }

    if registry.supported_extensions().is_empty() {
        eprintln!("No grammars installed. Run:");
        eprintln!();
        eprintln!("  eagraph grammars add python typescript  # or whichever languages you need");
        std::process::exit(1);
    }

    let repos: Vec<_> = config
        .repos
        .iter()
        .filter(|r| repo_filter.is_none_or(|f| r.name == f))
        .collect();

    if repos.is_empty() {
        if let Some(name) = repo_filter {
            anyhow::bail!("repo '{}' not found in config", name);
        }
        println!("No repos configured.");
        return Ok(());
    }

    for repo_config in &repos {
        println!("Indexing {}...", repo_config.name);
        match indexer::index_repo(
            repo_config,
            data_dir,
            &config.organization.name,
            registry,
            force,
        ) {
            Ok(result) => {
                println!(
                    "  {} files indexed, {} skipped, {} symbols, {} edges",
                    result.files_indexed,
                    result.files_skipped,
                    result.symbols_count,
                    result.edges_count,
                );
            }
            Err(e) => {
                eprintln!("  error: {}", e);
            }
        }
    }
    Ok(())
}

fn cmd_status(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    json: bool,
) -> Result<()> {
    let mut items = Vec::new();
    for repo_config in &config.repos {
        let (branch, db_path) = match config_loader::resolve_db_path(
            data_dir,
            &config.organization.name,
            &repo_config.name,
            &repo_config.root,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "warning: skipping {}: branch detection failed: {}",
                    repo_config.name, e
                );
                continue;
            }
        };

        let symbol_count = if db_path.exists() {
            match SqliteGraphStore::open(&db_path) {
                Ok(store) => {
                    if let Err(e) = indexer::auto_refresh(&store, repo_config, registry) {
                        eprintln!("warning: {}: auto-refresh failed: {}", repo_config.name, e);
                    }
                    match store.search_symbols("", None) {
                        Ok(s) => Some(s.len()),
                        Err(e) => {
                            eprintln!(
                                "warning: {}: search_symbols failed: {}",
                                repo_config.name, e
                            );
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("warning: {}: failed to open DB: {}", repo_config.name, e);
                    None
                }
            }
        } else {
            None
        };

        items.push((&repo_config.name, branch, symbol_count));
    }

    if json {
        let json_items: Vec<serde_json::Value> = items.iter().map(|(name, branch, count)| {
            serde_json::json!({ "name": name, "branch": branch, "symbols": count })
        }).collect();
        println!("{}", serde_json::to_string(&json_items)?);
    } else {
        println!("Organization: {}", config.organization.name);
        println!();
        for (name, branch, count) in &items {
            match count {
                Some(n) => println!("  {} (branch: {}) — {} symbols", name, branch, n),
                None => println!("  {} (branch: {}) — not indexed", name, branch),
            }
        }
    }
    Ok(())
}

fn cmd_query(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    name: &str,
    repo_filter: Option<&str>,
    json: bool,
) -> Result<()> {
    let repos: Vec<_> = config
        .repos
        .iter()
        .filter(|r| repo_filter.is_none_or(|f| r.name == f))
        .collect();

    let mut all_results = Vec::new();
    for repo_config in &repos {
        let (_branch, db_path) = config_loader::resolve_db_path(
            data_dir,
            &config.organization.name,
            &repo_config.name,
            &repo_config.root,
        )?;

        if !db_path.exists() {
            continue;
        }

        let store = SqliteGraphStore::open(&db_path)?;

        indexer::auto_refresh(&store, repo_config, registry)?;

        let results = store.search_symbols(name, None)?;

        for sym in results {
            all_results.push((repo_config.name.clone(), sym));
        }
    }

    if json {
        let items: Vec<serde_json::Value> = all_results
            .iter()
            .map(|(repo, s)| {
                serde_json::json!({
                    "name": s.name, "kind": s.kind.to_string(),
                    "file": s.file_path.to_str(), "lines": [s.line_start, s.line_end],
                    "repo": repo,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&items)?);
    } else {
        if all_results.is_empty() {
            println!("No symbols found matching '{}'", name);
        }
        for (repo, sym) in &all_results {
            println!(
                "  {} {} {}:{}–{} [{}]",
                sym.kind,
                sym.name,
                sym.file_path.display(),
                sym.line_start,
                sym.line_end,
                repo,
            );
        }
    }
    Ok(())
}

/// Detect which configured repo contains the current working directory.
fn detect_repo_from_cwd(config: &eagraph_core::Config) -> Option<String> {
    let cwd = std::fs::canonicalize(std::env::current_dir().ok()?).ok()?;
    config.repos.iter().find_map(|repo| {
        let root = std::fs::canonicalize(&repo.root).ok()?;
        cwd.starts_with(&root).then(|| repo.name.clone())
    })
}

fn resolve_repo_name(config: &eagraph_core::Config, explicit: Option<&str>) -> Result<String> {
    if let Some(name) = explicit {
        return Ok(name.to_string());
    }
    match detect_repo_from_cwd(config) {
        Some(name) => Ok(name),
        None => {
            let names: Vec<&str> = config.repos.iter().map(|r| r.name.as_str()).collect();
            anyhow::bail!(
                "could not detect repo from current directory. Use --repo <name>.\nConfigured repos: {}",
                names.join(", ")
            );
        }
    }
}

fn open_repo_store(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    repo_name: &str,
) -> Result<(eagraph_core::RepoConfig, SqliteGraphStore)> {
    let repo_config = config
        .repos
        .iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| anyhow::anyhow!("repo '{}' not found in config", repo_name))?
        .clone();

    let (_branch, db_path) = config_loader::resolve_db_path(
        data_dir,
        &config.organization.name,
        &repo_config.name,
        &repo_config.root,
    )?;

    if !db_path.exists() {
        anyhow::bail!(
            "repo '{}' not indexed yet. Run: eagraph index {}",
            repo_name,
            repo_name
        );
    }

    let store = SqliteGraphStore::open(&db_path)?;

    // Auto-refresh stale files before returning
    indexer::auto_refresh(&store, &repo_config, registry)?;

    Ok((repo_config, store))
}

fn cmd_context(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    name: &str,
    repo_name: &str,
    depth: u32,
    json: bool,
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, registry, repo_name)?;

    let result = eagraph_retriever::get_context(&store, &repo_config.root, name, depth, 2)?;

    let ctx = match result {
        Some(c) => c,
        None => {
            if json {
                println!("null");
            } else {
                println!("No symbol found matching '{}'", name);
            }
            return Ok(());
        }
    };

    if json {
        print_context_json(&ctx);
        return Ok(());
    }

    print_context_entry(&ctx.root);

    if !ctx.edges.is_empty() {
        println!();
        println!("Edges:");
        let id_to_name: std::collections::HashMap<&eagraph_core::SymbolId, &str> =
            std::iter::once((&ctx.root.symbol.id, ctx.root.symbol.name.as_str()))
                .chain(
                    ctx.neighbors
                        .iter()
                        .map(|e| (&e.symbol.id, e.symbol.name.as_str())),
                )
                .collect();
        for edge in &ctx.edges {
            let src = id_to_name.get(&edge.source).unwrap_or(&"?");
            let tgt = id_to_name.get(&edge.target).unwrap_or(&"?");
            println!("  {} → {} ({})", src, tgt, edge.kind);
        }
    }

    if !ctx.neighbors.is_empty() {
        println!();
        println!("Neighbors ({}):", ctx.neighbors.len());
        for entry in &ctx.neighbors {
            println!();
            print_context_entry(entry);
        }
    }

    Ok(())
}

fn cmd_dependents(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    file: &str,
    repo_name: &str,
    depth: u32,
    json: bool,
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, registry, repo_name)?;
    let file_path = resolve_file_path(file, &repo_config.root);

    let results =
        eagraph_retriever::get_dependents(&store, &repo_config.root, &file_path, depth, 2)?;

    if json {
        let items: Vec<serde_json::Value> = results.iter().map(context_to_json).collect();
        println!("{}", serde_json::to_string(&items)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No dependents found for {}", file);
        return Ok(());
    }

    for ctx in &results {
        print_context_entry(&ctx.root);
        if !ctx.neighbors.is_empty() {
            println!("  Depended on by:");
            for entry in &ctx.neighbors {
                println!(
                    "    {} ({}) at {}:{}–{}",
                    entry.symbol.name,
                    entry.symbol.kind,
                    entry.symbol.file_path.display(),
                    entry.symbol.line_start,
                    entry.symbol.line_end,
                );
            }
        }
        println!();
    }

    Ok(())
}

fn cmd_symbols(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    file: &str,
    repo_name: &str,
    json: bool,
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, registry, repo_name)?;
    let file_path = resolve_file_path(file, &repo_config.root);

    let symbols = store.get_file_symbols(&file_path)?;

    // Filter out module-scope symbols
    let symbols: Vec<_> = symbols
        .into_iter()
        .filter(|s| s.kind != eagraph_core::SymbolKind::Module)
        .collect();

    if json {
        let items: Vec<serde_json::Value> = symbols
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "kind": s.kind.to_string(),
                    "file": s.file_path.to_str(),
                    "lines": [s.line_start, s.line_end],
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&items)?);
        return Ok(());
    }

    if symbols.is_empty() {
        println!("No symbols found in {}", file);
        return Ok(());
    }

    for s in &symbols {
        println!(
            "  {:<8} {:<40} {}–{}",
            s.kind, s.name, s.line_start, s.line_end
        );
    }

    Ok(())
}

fn cmd_chain(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    from_name: &str,
    to_name: &str,
    repo_name: &str,
    json: bool,
) -> Result<()> {
    let (_, store) = open_repo_store(config, data_dir, registry, repo_name)?;

    let from_sym = store
        .search_symbols(from_name, None)?
        .into_iter()
        .find(|s| s.name == from_name);
    let to_sym = store
        .search_symbols(to_name, None)?
        .into_iter()
        .find(|s| s.name == to_name);

    let (from, to) = match (from_sym, to_sym) {
        (Some(f), Some(t)) => (f, t),
        (None, _) => {
            if json {
                println!("null");
            } else {
                println!("Symbol '{}' not found", from_name);
            }
            return Ok(());
        }
        (_, None) => {
            if json {
                println!("null");
            } else {
                println!("Symbol '{}' not found", to_name);
            }
            return Ok(());
        }
    };

    let path = store.get_shortest_path(&from.id, &to.id)?;

    match path {
        Some(ids) => {
            let mut steps = Vec::new();
            for id in &ids {
                let sym = store.get_symbol(id)?;
                steps.push(sym);
            }

            if json {
                let items: Vec<serde_json::Value> = steps
                    .iter()
                    .map(|s| match s {
                        Some(s) => serde_json::json!({
                            "name": s.name, "kind": s.kind.to_string(),
                            "file": s.file_path.to_str(), "lines": [s.line_start, s.line_end],
                        }),
                        None => serde_json::json!(null),
                    })
                    .collect();
                println!("{}", serde_json::to_string(&items)?);
            } else {
                println!("Path ({} hops):", ids.len() - 1);
                for (i, sym) in steps.iter().enumerate() {
                    match sym {
                        Some(s) => {
                            let arrow = if i < ids.len() - 1 { " →" } else { "" };
                            println!(
                                "  {} ({}) at {}:{}–{}{}",
                                s.name,
                                s.kind,
                                s.file_path.display(),
                                s.line_start,
                                s.line_end,
                                arrow,
                            );
                        }
                        None => println!("  [unknown symbol]"),
                    }
                }
            }
        }
        None => {
            if json {
                println!("null");
            } else {
                println!("No path from '{}' to '{}'", from_name, to_name);
            }
        }
    }

    Ok(())
}

fn cmd_viz(
    config: &eagraph_core::Config,
    data_dir: &Path,
    registry: &eagraph_parser::LanguageRegistry,
    port: u16,
) -> Result<()> {
    let mut repos = Vec::new();
    for repo_config in &config.repos {
        match open_repo_store(config, data_dir, registry, &repo_config.name) {
            Ok((_, store)) => {
                let symbols = store.get_all_symbols()?;
                let edges = store.get_all_edges()?;
                println!(
                    "  {}: {} symbols, {} edges",
                    repo_config.name,
                    symbols.len(),
                    edges.len()
                );
                repos.push((repo_config.name.clone(), symbols, edges));
            }
            Err(_) => {
                eprintln!("  {}: not indexed, skipping", repo_config.name);
            }
        }
    }
    if repos.is_empty() {
        anyhow::bail!("no indexed repos found");
    }
    viz::serve(repos, port)
}

fn resolve_file_path(file: &str, repo_root: &Path) -> PathBuf {
    if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        repo_root.join(file)
    }
}

fn cmd_init(config_path: &Path, org: &str) -> Result<()> {
    if config_path.exists() {
        anyhow::bail!("Config already exists at {}", config_path.display());
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format!("[organization]\nname = \"{}\"\n", org,);
    std::fs::write(config_path, &content)?;
    println!("Created {}", config_path.display());
    println!();
    println!("Add repos with:");
    println!("  eagraph add /path/to/repo");
    Ok(())
}

fn cmd_add(
    config_path: &Path,
    path: &str,
    name_override: Option<&str>,
    grammars_dir: &Path,
) -> Result<()> {
    let repo_path =
        std::fs::canonicalize(path).with_context(|| format!("path not found: {}", path))?;

    // Derive name from folder or use override
    let name = match name_override {
        Some(n) => n.to_string(),
        None => repo_path
            .file_name()
            .and_then(|f| f.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("could not derive name from path. Use --name"))?,
    };

    // Validate name: alphanumeric, hyphens, underscores only
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "invalid repo name '{}'. Use letters, numbers, hyphens, underscores.",
            name
        );
    }

    if !config_path.exists() {
        eprintln!("Config file not found. Run:");
        eprintln!();
        eprintln!("  eagraph init <org-name>");
        std::process::exit(1);
    }

    if !repo_path.join(".git").exists() {
        anyhow::bail!("{} is not a git repo. eagraph requires git for branch detection, .gitignore, and change tracking.", repo_path.display());
    }

    let config = config_loader::load_config(config_path)?;
    if config.repos.iter().any(|r| r.name == name) {
        anyhow::bail!("repo '{}' already exists in config", name);
    }

    let (installed_exts, missing_langs) = scan_repo_extensions(&repo_path, grammars_dir);

    // Include ALL detected extensions — both installed and missing grammars.
    // This way, after installing a grammar later, the files are already in scope.
    let mut all_exts: std::collections::BTreeSet<String> = installed_exts.iter().cloned().collect();
    for (_, ext) in &missing_langs {
        all_exts.insert(ext.clone());
    }
    let include = if all_exts.is_empty() {
        String::new()
    } else {
        let patterns: Vec<String> = all_exts.iter().map(|e| format!("\"**/*.{}\"", e)).collect();
        format!("include = [{}]\n", patterns.join(", "))
    };

    let entry = format!(
        "\n[[repos]]\nname = \"{}\"\nroot = \"{}\"\n{}",
        name,
        repo_path.display(),
        include,
    );

    let mut file = std::fs::OpenOptions::new().append(true).open(config_path)?;
    std::io::Write::write_all(&mut file, entry.as_bytes())?;

    println!("Added '{}' at {}", name, repo_path.display());
    if !installed_exts.is_empty() {
        println!(
            "  include: {}",
            installed_exts
                .iter()
                .map(|e| format!("*.{}", e))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Recommend missing grammars
    if !missing_langs.is_empty() {
        let lang_names: Vec<&str> = missing_langs
            .iter()
            .map(|(lang, _)| lang.as_str())
            .collect();
        println!();
        println!("This repo also has files for languages without grammars installed:");
        for (lang, ext) in &missing_langs {
            println!("  {} (*.{})", lang, ext);
        }
        println!();
        println!("Install with:");
        println!("  eagraph grammars add {}", lang_names.join(" "));
    }

    // Index immediately
    let registry = eagraph_parser::LanguageRegistry::from_dir(grammars_dir)?;

    if registry.supported_extensions().is_empty() {
        println!();
        println!("No grammars installed. Run:");
        println!("  eagraph grammars add python typescript  # or whichever you need");
        return Ok(());
    }

    let data_dir = config_loader::resolve_data_dir(None)?;
    // Reload config to include the newly added repo
    let config = config_loader::load_config(config_path)?;
    let repo_config = config
        .repos
        .iter()
        .find(|r| r.name == name)
        .expect("just added");

    println!();
    println!("Indexing {}...", name);
    match indexer::index_repo(
        repo_config,
        &data_dir,
        &config.organization.name,
        &registry,
        false,
    ) {
        Ok(result) => {
            println!(
                "  {} files indexed, {} skipped, {} symbols, {} edges",
                result.files_indexed,
                result.files_skipped,
                result.symbols_count,
                result.edges_count,
            );
        }
        Err(e) => eprintln!("  indexing failed: {}", e),
    }

    Ok(())
}

/// Scan a repo for all file extensions. Returns (installed, missing) where
/// installed = extensions with grammars, missing = extensions in the registry but not installed.
fn scan_repo_extensions(
    repo_path: &Path,
    grammars_dir: &Path,
) -> (Vec<String>, Vec<(String, String)>) {
    let installed = grammar_builder::all_supported_extensions(grammars_dir);
    let ext_to_lang = grammar_builder::bundled_ext_to_lang();

    let mut found_installed = std::collections::BTreeSet::new();
    let mut found_missing = std::collections::BTreeMap::<String, String>::new();

    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if installed.contains(ext) {
                found_installed.insert(ext.to_string());
            } else if let Some(lang) = ext_to_lang.get(ext) {
                found_missing.insert(lang.clone(), ext.to_string());
            }
        }
    }

    (
        found_installed.into_iter().collect(),
        found_missing.into_iter().collect(),
    )
}

// --- Output helpers ---

fn print_context_entry(entry: &eagraph_retriever::ContextEntry) {
    println!("## {} ({})", entry.symbol.name, entry.symbol.kind,);
    println!(
        "   {}:{}–{}",
        entry.symbol.file_path.display(),
        entry.symbol.line_start,
        entry.symbol.line_end,
    );
    if !entry.snippet.is_empty() {
        println!();
        for line in entry.snippet.lines() {
            println!("    {}", line);
        }
    }
}

fn entry_to_json(entry: &eagraph_retriever::ContextEntry) -> serde_json::Value {
    serde_json::json!({
        "name": entry.symbol.name,
        "kind": entry.symbol.kind.to_string(),
        "file": entry.symbol.file_path.to_str(),
        "lines": [entry.symbol.line_start, entry.symbol.line_end],
        "snippet": entry.snippet,
    })
}

fn context_to_json(ctx: &eagraph_retriever::ContextResult) -> serde_json::Value {
    let edges: Vec<serde_json::Value> = ctx
        .edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "source": e.source.0,
                "target": e.target.0,
                "kind": e.kind.to_string(),
            })
        })
        .collect();

    let neighbors: Vec<serde_json::Value> = ctx.neighbors.iter().map(entry_to_json).collect();

    serde_json::json!({
        "root": entry_to_json(&ctx.root),
        "neighbors": neighbors,
        "edges": edges,
    })
}

fn print_context_json(ctx: &eagraph_retriever::ContextResult) {
    println!(
        "{}",
        serde_json::to_string(&context_to_json(ctx)).expect("failed to serialize context")
    );
}

fn cmd_config(config_path: &Path, grammars_dir: &Path) -> Result<()> {
    println!("Config path:   {}", config_path.display());
    println!("Grammars dir:  {}", grammars_dir.display());
    println!();
    if config_path.exists() {
        let content = std::fs::read_to_string(config_path)?;
        print!("{}", content);
    } else {
        println!("Config file does not exist yet. Create it at:");
        println!("  {}", config_path.display());
        println!();
        println!("Example:");
        println!();
        println!("  [organization]");
        println!("  name = \"myorg\"");
        println!();
        println!("  [[repos]]");
        println!("  name = \"my-project\"");
        println!("  root = \"/path/to/my-project\"");
        println!("  include = [\"**/*.py\"]");
    }
    Ok(())
}
