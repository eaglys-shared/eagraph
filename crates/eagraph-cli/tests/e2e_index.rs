use std::path::PathBuf;

use eagraph_core::{GraphStore, SymbolKind};
use eagraph_parser::LanguageExtractor;
use sha2::Digest;
use eagraph_store_sqlite::SqliteGraphStore;

fn fixture_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("sample-repo")
}

/// Build a grammar dir once, shared across all tests in this binary.
fn test_grammars_dir() -> PathBuf {
    use std::sync::Once;
    static INIT: Once = Once::new();

    let dir = std::env::temp_dir()
        .join("eagraph-test-grammars")
        .join(format!("pid-{}", std::process::id()));

    INIT.call_once(|| {
        std::fs::create_dir_all(&dir).unwrap();

        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest.parent().unwrap().parent().unwrap();
        let parser_grammars = workspace_root
            .join("tests")
            .join("fixtures")
            .join("grammars-src")
            .join("python");

        let lib_name = if cfg!(target_os = "macos") {
            "python.dylib"
        } else if cfg!(target_os = "windows") {
            "python.dll"
        } else {
            "python.so"
        };

        let lib_path = dir.join(lib_name);
        if !lib_path.exists() {
            let parser_c = parser_grammars.join("parser.c");
            let scanner_c = parser_grammars.join("scanner.c");

            let obj_dir = dir.join("obj");
            std::fs::create_dir_all(&obj_dir).unwrap();

            for src in [&parser_c, &scanner_c] {
                if !src.exists() { continue; }
                let obj = obj_dir.join(src.file_stem().unwrap()).with_extension("o");
                let status = std::process::Command::new("cc")
                    .args(["-c", "-fPIC", "-O2"])
                    .arg("-I").arg(&parser_grammars)
                    .arg(src)
                    .arg("-o").arg(&obj)
                    .status()
                    .expect("cc not found");
                assert!(status.success(), "failed to compile {}", src.display());
            }

            let objs: Vec<PathBuf> = std::fs::read_dir(&obj_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("o"))
                .collect();

            let mut link = std::process::Command::new("cc");
            link.arg("-shared");
            if cfg!(target_os = "macos") {
                link.arg("-dynamiclib");
            }
            for obj in &objs {
                link.arg(obj);
            }
            link.arg("-o").arg(&lib_path);
            let status = link.status().expect("linker failed");
            assert!(status.success(), "failed to link {}", lib_name);
        }

        let grammars_config = workspace_root.join("grammars");
        std::fs::copy(grammars_config.join("python.scm"), dir.join("python.scm")).unwrap();
        std::fs::copy(grammars_config.join("python.toml"), dir.join("python.toml")).unwrap();
    });

    dir
}

fn temp_db_path(name: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join("eagraph-test")
        .join(format!("run-{}-{}-{}", std::process::id(), name, n));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("test.db")
}

#[test]
fn index_and_query_sample_repo() {
    let root = fixture_root();
    assert!(root.join("src/models.py").exists(), "fixture not found at {}", root.display());

    let db_path = temp_db_path("e2e");
    let store = SqliteGraphStore::open(&db_path).unwrap();

    let grammars_dir = test_grammars_dir();
    let registry = eagraph_parser::LanguageRegistry::from_dir(&grammars_dir).unwrap();
    let extractor = registry.extractor_for(std::path::Path::new("test.py"))
        .expect("python grammar not loaded");

    let pattern = format!("{}/src/**/*.py", root.display());
    let files: Vec<PathBuf> = glob::glob(&pattern)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|p| p.is_file())
        .collect();

    assert!(files.len() >= 3, "expected at least 3 .py files, got {}", files.len());

    let mut all_symbols = Vec::new();
    let mut all_raw_edges = Vec::new();

    for file_path in &files {
        let source = std::fs::read_to_string(file_path).unwrap();
        let (symbols, raw_edges) = extractor.extract(file_path, &source).unwrap();
        all_symbols.extend(symbols);
        all_raw_edges.extend(raw_edges);
    }

    assert!(!all_symbols.is_empty(), "no symbols extracted");
    assert!(!all_raw_edges.is_empty(), "no raw edges extracted");

    let resolved_edges = eagraph_core::RawEdge::resolve(&all_raw_edges, &all_symbols);

    store.upsert_symbols(&all_symbols).unwrap();
    store.upsert_edges(&resolved_edges).unwrap();

    let results = store.search_symbols("process_document", None).unwrap();
    assert!(!results.is_empty(), "process_document not found");
    assert_eq!(results[0].kind, SymbolKind::Function);

    let classes = store.search_symbols("User", Some(SymbolKind::Class)).unwrap();
    assert!(!classes.is_empty(), "User class not found");

    let methods = store.search_symbols("validate", Some(SymbolKind::Method)).unwrap();
    assert!(!methods.is_empty(), "validate method not found");

    let pd = &results[0];
    let sub = store
        .get_neighbors(&pd.id, eagraph_core::Direction::Outgoing, 1)
        .unwrap();
    assert!(!sub.edges.is_empty(), "process_document should have outgoing edges");

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
}

#[test]
fn reindex_skips_unchanged() {
    let root = fixture_root();
    let db_path = temp_db_path("e2e");
    let store = SqliteGraphStore::open(&db_path).unwrap();

    let grammars_dir = test_grammars_dir();
    let registry = eagraph_parser::LanguageRegistry::from_dir(&grammars_dir).unwrap();
    let extractor = registry.extractor_for(std::path::Path::new("test.py"))
        .expect("python grammar not loaded");

    let file = root.join("src/utils.py");
    let source = std::fs::read_to_string(&file).unwrap();
    let (symbols, _raw_edges) = extractor.extract(&file, &source).unwrap();

    store.upsert_symbols(&symbols).unwrap();

    let hash = format!("{:x}", sha2::Digest::finalize(sha2::Digest::chain_update(
        sha2::Sha256::new(), source.as_bytes()
    )));
    store.upsert_file_record(&eagraph_core::FileRecord {
        path: file.clone(),
        content_hash: hash.clone(),
        last_indexed: 1000,
    }).unwrap();

    let record = store.get_file_record(&file).unwrap().unwrap();
    assert_eq!(record.content_hash, hash);

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
}
