pub mod extractor;
pub mod grammar;
pub mod symbol_id;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::Path;

use eagraph_core::{EagraphError, RawEdge, Result, Symbol};

use extractor::{GenericExtractor, LanguageConfig};

/// Trait for language-specific symbol/edge extraction.
/// Returns symbols and unresolved edges (target is a name, not a resolved ID).
pub trait LanguageExtractor: Send + Sync {
    fn language_name(&self) -> &str;
    fn extract(&self, file_path: &Path, source: &str) -> Result<(Vec<Symbol>, Vec<RawEdge>)>;
}

/// Language registry: maps file extensions to extractors.
/// Built from a grammars directory containing .so/.dylib + .scm files.
pub struct LanguageRegistry {
    extractors: HashMap<String, GenericExtractor>,
}

impl LanguageRegistry {
    /// Build the registry by scanning a directory for grammar shared libraries
    /// and .scm query files.
    ///
    /// Expected layout:
    /// ```text
    /// grammars/
    ///   python.so (or .dylib)     — compiled tree-sitter grammar
    ///   python.scm                — query patterns
    ///   python.toml               — config (extensions, module_separator)
    ///   typescript.so
    ///   typescript.scm
    ///   typescript.toml
    /// ```
    ///
    /// Each language needs:
    /// - A shared library exporting `tree_sitter_{name}() -> *const TSLanguage`
    /// - A .scm file with query patterns using the capture naming convention
    /// - A .toml config with at minimum: `extensions = ["py", "pyi"]`
    pub fn from_dir(grammars_dir: &Path) -> Result<Self> {
        let mut extractors = HashMap::new();

        if !grammars_dir.exists() {
            return Ok(Self { extractors });
        }

        // Find all .toml config files — each one defines a language
        let entries = std::fs::read_dir(grammars_dir)
            .map_err(|e| EagraphError::Config(format!("reading {}: {}", grammars_dir.display(), e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| EagraphError::Config(e.to_string()))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            let lang_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if lang_name.is_empty() {
                continue;
            }

            match load_language(grammars_dir, &lang_name) {
                Ok((config, extensions)) => {
                    let extractor = match GenericExtractor::new(config) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("warning: skipping grammar '{}': {}", lang_name, e);
                            continue;
                        }
                    };
                    for ext in extensions {
                        extractors.insert(ext, extractor.clone());
                    }
                }
                Err(e) => {
                    eprintln!("warning: skipping grammar '{}': {}", lang_name, e);
                }
            }
        }

        Ok(Self { extractors })
    }

    /// Get an extractor for a file path based on its extension.
    pub fn extractor_for(&self, file_path: &Path) -> Option<&GenericExtractor> {
        let ext = file_path.extension()?.to_str()?;
        self.extractors.get(ext)
    }

    /// Check if a file extension is supported.
    pub fn supports(&self, file_path: &Path) -> bool {
        self.extractor_for(file_path).is_some()
    }

    /// List all supported extensions.
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.extractors.keys().map(|s| s.as_str()).collect()
    }

    /// Map of file extension → language name (e.g. "py" → "python", "ts" → "typescript").
    pub fn ext_to_lang(&self) -> std::collections::HashMap<String, String> {
        self.extractors
            .iter()
            .map(|(ext, e)| (ext.clone(), e.language_name().to_string()))
            .collect()
    }
}

/// Grammar config file format (the .toml next to the .so).
#[derive(serde::Deserialize)]
struct GrammarToml {
    extensions: Vec<String>,
    #[serde(default = "default_separator")]
    module_separator: String,
}

fn default_separator() -> String {
    ".".to_string()
}

/// Load a single language from the grammars directory.
fn load_language(
    dir: &Path,
    name: &str,
) -> std::result::Result<(LanguageConfig, Vec<String>), String> {
    // Load .toml config
    let toml_path = dir.join(format!("{}.toml", name));
    let toml_content = std::fs::read_to_string(&toml_path)
        .map_err(|e| format!("{}: {}", toml_path.display(), e))?;
    let config: GrammarToml = toml::from_str(&toml_content)
        .map_err(|e| format!("parsing {}: {}", toml_path.display(), e))?;

    // Load .scm queries
    let scm_path = dir.join(format!("{}.scm", name));
    let queries = std::fs::read_to_string(&scm_path)
        .map_err(|e| format!("{}: {}", scm_path.display(), e))?;

    // Load grammar shared library
    let ts_language = grammar::load_grammar(dir, name)?;

    Ok((
        LanguageConfig {
            name: name.to_string(),
            ts_language,
            queries,
            module_separator: config.module_separator,
        },
        config.extensions,
    ))
}

// --- Convenience API for tests ---

/// Parse a source file. Requires a LanguageRegistry.
pub fn parse_file_with(
    registry: &LanguageRegistry,
    file_path: &Path,
    source: &str,
) -> Result<(Vec<Symbol>, Vec<RawEdge>)> {
    match registry.extractor_for(file_path) {
        Some(ext) => ext.extract(file_path, source),
        None => Err(EagraphError::Parser(format!(
            "unsupported file type: {}",
            file_path.display()
        ))),
    }
}
