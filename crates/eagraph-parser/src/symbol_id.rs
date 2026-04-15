use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eagraph_core::SymbolId;

/// Generate a deterministic SymbolId from a UTF-8 file path, name, and kind.
/// Callers must validate the path is UTF-8 before calling (see `eagraph_core::path_to_str`).
pub fn make_symbol_id(file_path: &str, name: &str, kind: &str) -> SymbolId {
    let mut hasher = DefaultHasher::new();
    file_path.hash(&mut hasher);
    name.hash(&mut hasher);
    kind.hash(&mut hasher);
    SymbolId(format!("{:016x}", hasher.finish()))
}
