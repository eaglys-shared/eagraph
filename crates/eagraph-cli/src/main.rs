mod config_loader;
mod grammar_builder;
mod indexer;

use std::path::{Path, PathBuf};

use anyhow::Result;
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
        /// Repo name
        #[arg(long)]
        repo: String,
        /// Traversal depth
        #[arg(long, default_value = "2")]
        depth: u32,
    },
    /// Show what depends on symbols in a file
    Dependents {
        /// File path (relative to repo root)
        file: String,
        /// Repo name
        #[arg(long)]
        repo: String,
        /// Traversal depth
        #[arg(long, default_value = "1")]
        depth: u32,
    },
    /// List all symbols in a file
    Symbols {
        /// File path (relative to repo root)
        file: String,
        /// Repo name
        #[arg(long)]
        repo: String,
    },
    /// Find shortest call path between two symbols
    Chain {
        /// Source symbol name
        from: String,
        /// Target symbol name
        to: String,
        /// Repo name
        #[arg(long)]
        repo: String,
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
            GrammarsAction::Add { names } => return grammar_builder::cmd_grammars_add(names, &grammars_dir),
            GrammarsAction::List => return grammar_builder::cmd_grammars_list(&grammars_dir),
            GrammarsAction::Check => {} // needs config, fall through
        }
    }

    let config_path = config_loader::resolve_config_path(cli.config.as_deref())?;

    // Config command doesn't need a loaded config
    if let Command::Config = &cli.command {
        return cmd_config(&config_path, &grammars_dir);
    }

    if !config_path.exists() {
        eprintln!("Config file not found: {}", config_path.display());
        eprintln!();
        eprintln!("Create it with:");
        eprintln!();
        eprintln!("  [organization]");
        eprintln!("  name = \"myorg\"");
        eprintln!();
        eprintln!("  [[repos]]");
        eprintln!("  name = \"my-project\"");
        eprintln!("  root = \"/path/to/my-project\"");
        eprintln!("  include = [\"**/*.py\"]");
        std::process::exit(1);
    }
    let config = config_loader::load_config(&config_path)?;
    let data_dir = config_loader::resolve_data_dir(None)?;

    let json = cli.json;
    match cli.command {
        Command::Index { repo, all, force } => cmd_index(&config, &data_dir, &grammars_dir, repo.as_deref(), all, force),
        Command::Status => cmd_status(&config, &data_dir, json),
        Command::Query { name, repo } => cmd_query(&config, &data_dir, &name, repo.as_deref(), json),
        Command::Context { name, repo, depth } => cmd_context(&config, &data_dir, &name, &repo, depth, json),
        Command::Dependents { file, repo, depth } => cmd_dependents(&config, &data_dir, &file, &repo, depth, json),
        Command::Symbols { file, repo } => cmd_symbols(&config, &data_dir, &file, &repo, json),
        Command::Chain { from, to, repo } => cmd_chain(&config, &data_dir, &from, &to, &repo, json),
        Command::Grammars { action: GrammarsAction::Check } => {
            grammar_builder::cmd_grammars_check(&config, &grammars_dir)
        }
        Command::Config | Command::Grammars { .. } => unreachable!(),
    }
}

fn cmd_index(config: &eagraph_core::Config, data_dir: &PathBuf, grammars_dir: &PathBuf, repo_filter: Option<&str>, all: bool, force: bool) -> Result<()> {
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

    let registry = eagraph_parser::LanguageRegistry::from_dir(grammars_dir)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let extensions = registry.supported_extensions();
    if extensions.is_empty() {
        eprintln!("No grammars installed. Run:");
        eprintln!();
        eprintln!("  eagraph grammars add python typescript  # or whichever languages you need");
        std::process::exit(1);
    }

    let repos: Vec<_> = config
        .repos
        .iter()
        .filter(|r| repo_filter.map_or(true, |f| r.name == f))
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
        match indexer::index_repo(repo_config, data_dir, &config.organization.name, &registry, force) {
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

fn cmd_status(config: &eagraph_core::Config, data_dir: &PathBuf, json: bool) -> Result<()> {
    let mut items = Vec::new();
    for repo_config in &config.repos {
        let branch = config_loader::detect_branch(&repo_config.root)
            .unwrap_or_else(|_| "unknown".to_string());
        let db_path = config_loader::db_path(data_dir, &config.organization.name, &repo_config.name, &branch);

        let symbol_count = if db_path.exists() {
            SqliteGraphStore::open(&db_path)
                .ok()
                .and_then(|store| store.search_symbols("", None).ok())
                .map(|s| s.len())
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
    data_dir: &PathBuf,
    name: &str,
    repo_filter: Option<&str>,
    json: bool,
) -> Result<()> {
    let repos: Vec<_> = config
        .repos
        .iter()
        .filter(|r| repo_filter.map_or(true, |f| r.name == f))
        .collect();

    let mut all_results = Vec::new();
    for repo_config in &repos {
        let branch = config_loader::detect_branch(&repo_config.root)
            .unwrap_or_else(|_| "main".to_string());
        let db_path = config_loader::db_path(data_dir, &config.organization.name, &repo_config.name, &branch);

        if !db_path.exists() {
            continue;
        }

        let store = SqliteGraphStore::open(&db_path)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let results = store.search_symbols(name, None)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        for sym in results {
            all_results.push((repo_config.name.clone(), sym));
        }
    }

    if json {
        let items: Vec<serde_json::Value> = all_results.iter().map(|(repo, s)| {
            serde_json::json!({
                "name": s.name, "kind": s.kind.to_string(),
                "file": s.file_path.to_str(), "lines": [s.line_start, s.line_end],
                "repo": repo,
            })
        }).collect();
        println!("{}", serde_json::to_string(&items)?);
    } else {
        if all_results.is_empty() {
            println!("No symbols found matching '{}'", name);
        }
        for (repo, sym) in &all_results {
            println!(
                "  {} {} {}:{}–{} [{}]",
                sym.kind, sym.name, sym.file_path.display(),
                sym.line_start, sym.line_end, repo,
            );
        }
    }
    Ok(())
}

fn open_repo_store(
    config: &eagraph_core::Config,
    data_dir: &PathBuf,
    repo_name: &str,
) -> Result<(eagraph_core::RepoConfig, SqliteGraphStore)> {
    let repo_config = config
        .repos
        .iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| anyhow::anyhow!("repo '{}' not found in config", repo_name))?
        .clone();

    let branch = config_loader::detect_branch(&repo_config.root)
        .unwrap_or_else(|_| "main".to_string());
    let db_path = config_loader::db_path(data_dir, &config.organization.name, &repo_config.name, &branch);

    if !db_path.exists() {
        anyhow::bail!("repo '{}' not indexed yet. Run: eagraph index {}", repo_name, repo_name);
    }

    let store = SqliteGraphStore::open(&db_path)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    Ok((repo_config, store))
}

fn cmd_context(
    config: &eagraph_core::Config,
    data_dir: &PathBuf,
    name: &str,
    repo_name: &str,
    depth: u32,
    json: bool,
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, repo_name)?;

    let result = eagraph_retriever::get_context(&store, &repo_config.root, name, depth, 2)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

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
                .chain(ctx.neighbors.iter().map(|e| (&e.symbol.id, e.symbol.name.as_str())))
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
    data_dir: &PathBuf,
    file: &str,
    repo_name: &str,
    depth: u32,
    json: bool,
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, repo_name)?;
    let file_path = resolve_file_path(file, &repo_config.root);

    let results = eagraph_retriever::get_dependents(&store, &repo_config.root, &file_path, depth, 2)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if json {
        let items: Vec<serde_json::Value> = results.iter().map(|ctx| context_to_json(ctx)).collect();
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
    data_dir: &PathBuf,
    file: &str,
    repo_name: &str,
    json: bool,
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, repo_name)?;
    let file_path = resolve_file_path(file, &repo_config.root);

    let symbols = store.get_file_symbols(&file_path)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Filter out module-scope symbols
    let symbols: Vec<_> = symbols.into_iter().filter(|s| s.kind != eagraph_core::SymbolKind::Module).collect();

    if json {
        let items: Vec<serde_json::Value> = symbols.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "kind": s.kind.to_string(),
                "file": s.file_path.to_str(),
                "lines": [s.line_start, s.line_end],
            })
        }).collect();
        println!("{}", serde_json::to_string(&items)?);
        return Ok(());
    }

    if symbols.is_empty() {
        println!("No symbols found in {}", file);
        return Ok(());
    }

    for s in &symbols {
        println!("  {:<8} {:<40} {}–{}", s.kind, s.name, s.line_start, s.line_end);
    }

    Ok(())
}

fn cmd_chain(
    config: &eagraph_core::Config,
    data_dir: &PathBuf,
    from_name: &str,
    to_name: &str,
    repo_name: &str,
    json: bool,
) -> Result<()> {
    let (_, store) = open_repo_store(config, data_dir, repo_name)?;

    let from_sym = store.search_symbols(from_name, None)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .into_iter()
        .find(|s| s.name == from_name);
    let to_sym = store.search_symbols(to_name, None)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .into_iter()
        .find(|s| s.name == to_name);

    let (from, to) = match (from_sym, to_sym) {
        (Some(f), Some(t)) => (f, t),
        (None, _) => {
            if json { println!("null"); } else { println!("Symbol '{}' not found", from_name); }
            return Ok(());
        }
        (_, None) => {
            if json { println!("null"); } else { println!("Symbol '{}' not found", to_name); }
            return Ok(());
        }
    };

    let path = store.get_shortest_path(&from.id, &to.id)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    match path {
        Some(ids) => {
            let mut steps = Vec::new();
            for id in &ids {
                let sym = store.get_symbol(id).map_err(|e| anyhow::anyhow!("{}", e))?;
                steps.push(sym);
            }

            if json {
                let items: Vec<serde_json::Value> = steps.iter().map(|s| match s {
                    Some(s) => serde_json::json!({
                        "name": s.name, "kind": s.kind.to_string(),
                        "file": s.file_path.to_str(), "lines": [s.line_start, s.line_end],
                    }),
                    None => serde_json::json!(null),
                }).collect();
                println!("{}", serde_json::to_string(&items)?);
            } else {
                println!("Path ({} hops):", ids.len() - 1);
                for (i, sym) in steps.iter().enumerate() {
                    match sym {
                        Some(s) => {
                            let arrow = if i < ids.len() - 1 { " →" } else { "" };
                            println!(
                                "  {} ({}) at {}:{}–{}{}",
                                s.name, s.kind, s.file_path.display(), s.line_start, s.line_end, arrow,
                            );
                        }
                        None => println!("  [unknown symbol]"),
                    }
                }
            }
        }
        None => {
            if json { println!("null"); } else { println!("No path from '{}' to '{}'", from_name, to_name); }
        }
    }

    Ok(())
}

fn resolve_file_path(file: &str, repo_root: &Path) -> PathBuf {
    if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        repo_root.join(file)
    }
}

// --- Output helpers ---

fn print_context_entry(entry: &eagraph_retriever::ContextEntry) {
    println!(
        "## {} ({})",
        entry.symbol.name, entry.symbol.kind,
    );
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
    let edges: Vec<serde_json::Value> = ctx.edges.iter().map(|e| {
        serde_json::json!({
            "source": e.source.0,
            "target": e.target.0,
            "kind": e.kind.to_string(),
        })
    }).collect();

    let neighbors: Vec<serde_json::Value> = ctx.neighbors.iter().map(entry_to_json).collect();

    serde_json::json!({
        "root": entry_to_json(&ctx.root),
        "neighbors": neighbors,
        "edges": edges,
    })
}

fn print_context_json(ctx: &eagraph_retriever::ContextResult) {
    println!("{}", serde_json::to_string(&context_to_json(ctx)).expect("failed to serialize context"));
}

fn cmd_config(config_path: &PathBuf, grammars_dir: &PathBuf) -> Result<()> {
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
