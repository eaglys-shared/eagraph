mod config_loader;
mod grammar_builder;
mod indexer;

use std::path::PathBuf;

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
    /// Index all repos (or a single repo)
    Index {
        /// Index only this repo
        #[arg(long)]
        repo: Option<String>,
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let grammars_dir = config_loader::resolve_grammars_dir(None)?;

    // Grammars commands don't need config.toml
    if let Command::Grammars { action } = &cli.command {
        return match action {
            GrammarsAction::Add { names } => grammar_builder::cmd_grammars_add(names, &grammars_dir),
            GrammarsAction::List => grammar_builder::cmd_grammars_list(&grammars_dir),
        };
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
        Command::Index { repo, force } => cmd_index(&config, &data_dir, &grammars_dir, repo.as_deref(), force),
        Command::Status => cmd_status(&config, &data_dir),
        Command::Query { name, repo } => cmd_query(&config, &data_dir, &name, repo.as_deref()),
        Command::Config | Command::Grammars { .. } => unreachable!(),
    }
}

fn cmd_index(config: &eagraph_core::Config, data_dir: &PathBuf, grammars_dir: &PathBuf, repo_filter: Option<&str>, force: bool) -> Result<()> {
    let registry = eagraph_parser::LanguageRegistry::from_dir(grammars_dir)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let extensions = registry.supported_extensions();
    if extensions.is_empty() {
        println!("No grammars found in {}. Add .so + .scm + .toml files to enable parsing.", grammars_dir.display());
        return Ok(());
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
