use std::path::Path;

use tree_sitter::Language;

/// Load a tree-sitter grammar from a shared library.
///
/// Looks for `{dir}/{name}.so` (Linux), `{dir}/{name}.dylib` (macOS),
/// or `{dir}/{name}.dll` (Windows).
///
/// The library must export a C function `tree_sitter_{name}` that returns
/// a `*const TSLanguage`. This is the standard convention for tree-sitter
/// grammar crates.
pub fn load_grammar(dir: &Path, name: &str) -> Result<Language, String> {
    let lib_path = find_library(dir, name)?;

    // SAFETY: We're loading a tree-sitter grammar library that exports
    // the standard `tree_sitter_{name}` symbol. The returned pointer is
    // a valid TSLanguage that tree-sitter will manage.
    unsafe {
        let lib = libloading::Library::new(&lib_path)
            .map_err(|e| format!("loading {}: {}", lib_path.display(), e))?;

        let func_name = format!("tree_sitter_{}", name);
        let func: libloading::Symbol<unsafe extern "C" fn() -> *const std::ffi::c_void> = lib
            .get(func_name.as_bytes())
            .map_err(|e| format!("symbol '{}' in {}: {}", func_name, lib_path.display(), e))?;

        let raw_ptr = func();
        if raw_ptr.is_null() {
            return Err(format!("{}() returned null", func_name));
        }

        // Deliberately leak the library so the language pointer stays valid
        // for the lifetime of the process.
        std::mem::forget(lib);

        // Construct Language from raw pointer.
        // Language is repr(transparent) around *const TSLanguage.
        let language: Language = std::mem::transmute(raw_ptr);

        Ok(language)
    }
}

fn find_library(dir: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let candidates = if cfg!(target_os = "macos") {
        vec![
            dir.join(format!("{}.dylib", name)),
            dir.join(format!("lib{}.dylib", name)),
            dir.join(format!("{}.so", name)),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            dir.join(format!("{}.dll", name)),
        ]
    } else {
        vec![
            dir.join(format!("{}.so", name)),
            dir.join(format!("lib{}.so", name)),
        ]
    };

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(format!(
        "no shared library for '{}' in {}. Expected one of: {}",
        name,
        dir.display(),
        candidates
            .iter()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}
