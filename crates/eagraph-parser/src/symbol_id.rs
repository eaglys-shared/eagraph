use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use eagraph_core::SymbolId;

/// Generate a deterministic SymbolId from file path, name, and kind.
pub fn make_symbol_id(file_path: &Path, name: &str, kind: &str) -> SymbolId {
    let mut hasher = DefaultHasher::new();
    file_path.to_str().unwrap_or("").hash(&mut hasher);
    name.hash(&mut hasher);
    kind.hash(&mut hasher);
    SymbolId(format!("{:016x}", hasher.finish()))
}
