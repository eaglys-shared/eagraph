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

    match cli.command {
        Command::Index { repo, all, force } => cmd_index(&config, &data_dir, &grammars_dir, repo.as_deref(), all, force),
        Command::Status => cmd_status(&config, &data_dir),
        Command::Query { name, repo } => cmd_query(&config, &data_dir, &name, repo.as_deref()),
        Command::Context { name, repo, depth } => cmd_context(&config, &data_dir, &name, &repo, depth),
        Command::Dependents { file, repo, depth } => cmd_dependents(&config, &data_dir, &file, &repo, depth),
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

fn cmd_status(config: &eagraph_core::Config, data_dir: &PathBuf) -> Result<()> {
    println!("Organization: {}", config.organization.name);
    println!();

    for repo_config in &config.repos {
        let branch = config_loader::detect_branch(&repo_config.root)
            .unwrap_or_else(|_| "unknown".to_string());
        let db_path = config_loader::db_path(data_dir, &config.organization.name, &repo_config.name, &branch);

        print!("  {} (branch: {})", repo_config.name, branch);

        if db_path.exists() {
            match SqliteGraphStore::open(&db_path) {
                Ok(store) => {
                    let symbols = store.search_symbols("", None)
                        .map(|s| s.len())
                        .unwrap_or(0);
                    println!(" — {} symbols", symbols);
                }
                Err(_) => println!(" — db error"),
            }
        } else {
            println!(" — not indexed");
        }
    }
    Ok(())
}

fn cmd_query(
    config: &eagraph_core::Config,
    data_dir: &PathBuf,
    name: &str,
    repo_filter: Option<&str>,
) -> Result<()> {
    let repos: Vec<_> = config
        .repos
        .iter()
        .filter(|r| repo_filter.map_or(true, |f| r.name == f))
        .collect();

    let mut found = false;
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

        for sym in &results {
            found = true;
            println!(
                "  {} {} {}:{}–{} [{}]",
                sym.kind,
                sym.name,
                sym.file_path.display(),
                sym.line_start,
                sym.line_end,
                repo_config.name,
            );
        }
    }

    if !found {
        println!("No symbols found matching '{}'", name);
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
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, repo_name)?;

    let result = eagraph_retriever::get_context(&store, &repo_config.root, name, depth, 2)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let ctx = match result {
        Some(c) => c,
        None => {
            println!("No symbol found matching '{}'", name);
            return Ok(());
        }
    };

    // Print root symbol
    print_context_entry(&ctx.root, &repo_config.name);

    // Print edges grouped by kind
    if !ctx.edges.is_empty() {
        println!();
        println!("Edges:");
        for edge in &ctx.edges {
            let source_name = std::iter::once(&ctx.root)
                .chain(ctx.neighbors.iter())
                .find(|e| e.symbol.id == edge.source)
                .map(|e| e.symbol.name.as_str())
                .unwrap_or("?");
            let target_name = std::iter::once(&ctx.root)
                .chain(ctx.neighbors.iter())
                .find(|e| e.symbol.id == edge.target)
                .map(|e| e.symbol.name.as_str())
                .unwrap_or("?");
            println!("  {} → {} ({})", source_name, target_name, edge.kind);
        }
    }

    // Print neighbor snippets
    if !ctx.neighbors.is_empty() {
        println!();
        println!("Neighbors ({}):", ctx.neighbors.len());
        for entry in &ctx.neighbors {
            println!();
            print_context_entry(entry, &repo_config.name);
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
) -> Result<()> {
    let (repo_config, store) = open_repo_store(config, data_dir, repo_name)?;
    // Resolve to absolute path for DB lookup
    let file_path = if Path::new(file).is_absolute() {
        PathBuf::from(file)
    } else {
        repo_config.root.join(file)
    };

    let results = eagraph_retriever::get_dependents(&store, &repo_config.root, &file_path, depth, 2)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if results.is_empty() {
        println!("No dependents found for {}", file);
        return Ok(());
    }

    for ctx in &results {
        print_context_entry(&ctx.root, &repo_config.name);

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

fn print_context_entry(entry: &eagraph_retriever::ContextEntry, repo_name: &str) {
    println!(
        "## {} ({}) [{}]",
        entry.symbol.name, entry.symbol.kind, repo_name,
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
