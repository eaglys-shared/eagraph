use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct RegistryEntry {
    repo: String,
    src_subdir: Option<String>,
}

type Registry = BTreeMap<String, RegistryEntry>;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR has no grandparent")
        .to_path_buf()
}

fn load_registry() -> Result<Registry> {
    let registry_path = workspace_root().join("grammars").join("registry.toml");
    let content = std::fs::read_to_string(&registry_path)
        .with_context(|| format!("reading {}", registry_path.display()))?;
    let registry: Registry = toml::from_str(&content)
        .with_context(|| format!("parsing {}", registry_path.display()))?;
    Ok(registry)
}

fn bundled_grammars_dir() -> PathBuf {
    workspace_root().join("grammars")
}

pub fn cmd_grammars_add(names: &[String], grammars_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(grammars_dir)
        .with_context(|| format!("creating {}", grammars_dir.display()))?;

    check_compiler()?;

    let registry = load_registry()?;

    for name in names {
        match registry.get(name.as_str()) {
            Some(entry) => {
                println!("Building {}...", name);
                match build_grammar(name, entry, grammars_dir) {
                    Ok(()) => println!("  installed to {}", grammars_dir.display()),
                    Err(e) => eprintln!("  failed: {}", e),
                }
            }
            None => {
                eprintln!("'{}' not in registry. Known grammars:", name);
                for key in registry.keys() {
                    eprintln!("  {}", key);
                }
                eprintln!();
                eprintln!("To add a custom grammar, build it manually. See README.md.");
            }
        }
    }
    Ok(())
}

pub fn cmd_grammars_list(grammars_dir: &Path) -> Result<()> {
    let registry = load_registry()?;
    let bundled = bundled_grammars_dir();

    println!("Known grammars:");
    for (name, _entry) in &registry {
        let installed = is_installed(grammars_dir, name);

        // Read extensions from the bundled .toml
        let extensions = read_extensions(&bundled, name);
        let ext_str = extensions.join(", ");

        let status = if installed { "installed" } else { "not installed" };
        println!("  {:<14} extensions: {:<24} {}", name, ext_str, status);
    }
    Ok(())
}

fn is_installed(grammars_dir: &Path, name: &str) -> bool {
    let dylib = grammars_dir.join(format!("{}.dylib", name));
    let so = grammars_dir.join(format!("{}.so", name));
    let dll = grammars_dir.join(format!("{}.dll", name));
    dylib.exists() || so.exists() || dll.exists()
}

fn read_extensions(bundled_dir: &Path, name: &str) -> Vec<String> {
    let toml_path = bundled_dir.join(format!("{}.toml", name));
    if let Ok(content) = std::fs::read_to_string(&toml_path) {
        #[derive(Deserialize)]
        struct GrammarToml {
            #[serde(default)]
            extensions: Vec<String>,
        }
        if let Ok(parsed) = toml::from_str::<GrammarToml>(&content) {
            return parsed.extensions;
        }
    }
    vec![]
}

fn build_grammar(name: &str, entry: &RegistryEntry, grammars_dir: &Path) -> Result<()> {
    let tmp = std::env::temp_dir()
        .join("eagraph-grammar-build")
        .join(name);
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp)?;
    }

    // Clone
    let repo_url = format!("https://github.com/{}.git", entry.repo);
    println!("  cloning {}...", repo_url);
    let status = Command::new("git")
        .args(["clone", "--depth", "1", &repo_url])
        .arg(&tmp)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .status()
        .context("git clone failed")?;
    if !status.success() {
        bail!("git clone failed for {}", repo_url);
    }

    // Find src directory
    let src_dir = match &entry.src_subdir {
        Some(sub) => tmp.join(sub).join("src"),
        None => tmp.join("src"),
    };
    if !src_dir.join("parser.c").exists() {
        bail!("parser.c not found in {}", src_dir.display());
    }

    // Auto-detect source files
    println!("  compiling...");
    let mut sources = vec![src_dir.join("parser.c")];
    let has_cc = src_dir.join("scanner.cc").exists();
    if src_dir.join("scanner.c").exists() {
        sources.push(src_dir.join("scanner.c"));
    }
    if has_cc {
        sources.push(src_dir.join("scanner.cc"));
    }

    let lib_name = if cfg!(target_os = "macos") {
        format!("{}.dylib", name)
    } else if cfg!(target_os = "windows") {
        format!("{}.dll", name)
    } else {
        format!("{}.so", name)
    };
    let output_path = grammars_dir.join(&lib_name);

    // Compile each source to .o
    let obj_dir = tmp.join("obj");
    std::fs::create_dir_all(&obj_dir)?;
    let mut objects = Vec::new();

    for src in &sources {
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("c");
        let cc = if ext == "cc" || ext == "cpp" { "c++" } else { "cc" };
        let stem = src.file_stem().context("source file has no stem")?;
        let obj = obj_dir.join(stem).with_extension("o");
        let status = Command::new(cc)
            .args(["-c", "-fPIC", "-O2"])
            .arg("-I")
            .arg(&src_dir)
            .arg(src)
            .arg("-o")
            .arg(&obj)
            .status()
            .with_context(|| format!("compiling {}", src.display()))?;
        if !status.success() {
            bail!("compilation failed: {}", src.display());
        }
        objects.push(obj);
    }

    // Link
    let compiler = if has_cc { "c++" } else { "cc" };
    let mut link = Command::new(compiler);
    link.arg("-shared");
    if cfg!(target_os = "macos") {
        link.arg("-dynamiclib");
    }
    for obj in &objects {
        link.arg(obj);
    }
    if has_cc {
        link.arg("-lstdc++");
    }
    link.arg("-o").arg(&output_path);
    let status = link.status().context("linking failed")?;
    if !status.success() {
        bail!("linking failed for {}", lib_name);
    }

    // Copy .scm and .toml from bundled grammars/
    let bundled = bundled_grammars_dir();
    for ext in ["scm", "toml"] {
        let src = bundled.join(format!("{}.{}", name, ext));
        let dst = grammars_dir.join(format!("{}.{}", name, ext));
        if src.exists() {
            std::fs::copy(&src, &dst)?;
        } else {
            bail!(
                "missing {}.{} in {}",
                name, ext, bundled.display()
            );
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp);

    Ok(())
}

pub fn cmd_grammars_check(config: &eagraph_core::Config, grammars_dir: &Path) -> Result<()> {
    let registry = load_registry()?;
    let bundled = bundled_grammars_dir();

    // Build extension → language name map from registry
    let mut ext_to_lang: BTreeMap<String, String> = BTreeMap::new();
    for (name, _) in &registry {
        for ext in read_extensions(&bundled, name) {
            ext_to_lang.insert(ext, name.clone());
        }
    }

    // Scan all repo roots for file extensions
    let mut found_extensions: BTreeMap<String, usize> = BTreeMap::new();
    for repo in &config.repos {
        if !repo.root.exists() {
            continue;
        }
        let walker = ignore::WalkBuilder::new(&repo.root)
            .hidden(true)
            .git_ignore(true)
            .build();
        for entry in walker.flatten() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                *found_extensions.entry(ext.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Match found extensions against known languages
    let mut need_install: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();
    let mut covered: BTreeMap<String, Vec<(String, usize)>> = BTreeMap::new();

    for (ext, count) in &found_extensions {
        if let Some(lang) = ext_to_lang.get(ext) {
            if is_installed(grammars_dir, lang) {
                covered
                    .entry(lang.clone())
                    .or_default()
                    .push((ext.clone(), *count));
            } else {
                need_install
                    .entry(lang.clone())
                    .or_default()
                    .push((ext.clone(), *count));
            }
        }
        // Don't report unknown extensions — too noisy (images, configs, etc.)
    }

    if !covered.is_empty() {
        println!("Covered (grammar installed):");
        for (lang, exts) in &covered {
            let detail: Vec<String> = exts.iter().map(|(e, c)| format!(".{} ({})", e, c)).collect();
            println!("  {:<14} {}", lang, detail.join(", "));
        }
        println!();
    }

    if !need_install.is_empty() {
        println!("Missing (grammar available but not installed):");
        for (lang, exts) in &need_install {
            let detail: Vec<String> = exts.iter().map(|(e, c)| format!(".{} ({})", e, c)).collect();
            println!("  {:<14} {}", lang, detail.join(", "));
        }
        println!();
        let names: Vec<&str> = need_install.keys().map(|s| s.as_str()).collect();
        println!("Install with:");
        println!("  eagraph grammars add {}", names.join(" "));
    } else if covered.is_empty() {
        println!("No recognized source files found in configured repos.");
    } else {
        println!("All detected languages have grammars installed.");
    }

    Ok(())
}

fn check_compiler() -> Result<()> {
    let status = Command::new("cc")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => bail!(
            "C compiler not found. Install one:\n\
             \n  macOS:  xcode-select --install\
             \n  Ubuntu: sudo apt install build-essential\
             \n  Fedora: sudo dnf install gcc"
        ),
    }
}
