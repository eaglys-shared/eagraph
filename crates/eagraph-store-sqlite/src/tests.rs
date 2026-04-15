use std::path::PathBuf;

use eagraph_core::*;

use crate::SqliteGraphStore;

fn make_store() -> SqliteGraphStore {
    SqliteGraphStore::open_in_memory().unwrap()
}

fn sym(id: &str, name: &str, kind: SymbolKind, file: &str, start: u32, end: u32) -> Symbol {
    Symbol {
        id: SymbolId(id.to_string()),
        name: name.to_string(),
        kind,
        file_path: PathBuf::from(file),
        line_start: start,
        line_end: end,
        metadata: None,
    }
}

fn edge(source: &str, target: &str, kind: EdgeKind) -> Edge {
    Edge {
        source: SymbolId(source.to_string()),
        target: SymbolId(target.to_string()),
        kind,
    }
}

// ---- Symbol CRUD ----

#[test]
fn upsert_and_get_symbol() {
    let store = make_store();
    let s = sym("s1", "foo", SymbolKind::Function, "src/main.py", 1, 10);
    store.upsert_symbols(std::slice::from_ref(&s)).unwrap();

    let got = store.get_symbol(&SymbolId("s1".into())).unwrap().unwrap();
    assert_eq!(got.name, "foo");
    assert_eq!(got.kind, SymbolKind::Function);
    assert_eq!(got.file_path, PathBuf::from("src/main.py"));
    assert_eq!(got.line_start, 1);
    assert_eq!(got.line_end, 10);
}

#[test]
fn upsert_symbol_updates_on_conflict() {
    let store = make_store();
    let s1 = sym("s1", "foo", SymbolKind::Function, "src/main.py", 1, 10);
    store.upsert_symbols(&[s1]).unwrap();

    let s1_updated = sym("s1", "foo_v2", SymbolKind::Method, "src/main.py", 5, 20);
    store.upsert_symbols(&[s1_updated]).unwrap();

    let got = store.get_symbol(&SymbolId("s1".into())).unwrap().unwrap();
    assert_eq!(got.name, "foo_v2");
    assert_eq!(got.kind, SymbolKind::Method);
    assert_eq!(got.line_start, 5);
}

#[test]
fn get_symbol_not_found() {
    let store = make_store();
    let got = store.get_symbol(&SymbolId("nope".into())).unwrap();
    assert!(got.is_none());
}

// ---- Edge CRUD ----

#[test]
fn upsert_and_get_edges_via_neighbors() {
    let store = make_store();
    let s1 = sym("s1", "caller", SymbolKind::Function, "a.py", 1, 5);
    let s2 = sym("s2", "callee", SymbolKind::Function, "b.py", 1, 5);
    store.upsert_symbols(&[s1, s2]).unwrap();

    let e = edge("s1", "s2", EdgeKind::Calls);
    store.upsert_edges(&[e]).unwrap();

    let sub = store
        .get_neighbors(&SymbolId("s1".into()), Direction::Outgoing, 1)
        .unwrap();
    assert_eq!(sub.symbols.len(), 1);
    assert_eq!(sub.symbols[0].name, "callee");
    assert_eq!(sub.edges.len(), 1);
    assert_eq!(sub.edges[0].kind, EdgeKind::Calls);
}

#[test]
fn upsert_edge_deduplicates() {
    let store = make_store();
    let s1 = sym("s1", "a", SymbolKind::Function, "a.py", 1, 1);
    let s2 = sym("s2", "b", SymbolKind::Function, "a.py", 2, 2);
    store.upsert_symbols(&[s1, s2]).unwrap();

    let e = edge("s1", "s2", EdgeKind::Calls);
    store.upsert_edges(&[e.clone(), e]).unwrap();

    let sub = store
        .get_neighbors(&SymbolId("s1".into()), Direction::Outgoing, 1)
        .unwrap();
    assert_eq!(sub.edges.len(), 1);
}

// ---- delete_file_data ----

#[test]
fn delete_file_data_removes_symbols_edges_files() {
    let store = make_store();
    let s1 = sym("s1", "a", SymbolKind::Function, "target.py", 1, 5);
    let s2 = sym("s2", "b", SymbolKind::Function, "other.py", 1, 5);
    store.upsert_symbols(&[s1, s2]).unwrap();
    store
        .upsert_edges(&[edge("s1", "s2", EdgeKind::Calls)])
        .unwrap();
    store
        .upsert_file_record(&FileRecord {
            path: PathBuf::from("target.py"),
            content_hash: "abc".into(),
            last_indexed: 100,
        })
        .unwrap();
    store
        .upsert_annotations(&[Annotation {
            symbol_id: SymbolId("s1".into()),
            source: "test".into(),
            key: "k".into(),
            value: "v".into(),
        }])
        .unwrap();

    store.delete_file_data(&PathBuf::from("target.py")).unwrap();

    assert!(store.get_symbol(&SymbolId("s1".into())).unwrap().is_none());
    assert!(store
        .get_file_record(&PathBuf::from("target.py"))
        .unwrap()
        .is_none());
    assert!(store
        .get_annotations(&SymbolId("s1".into()))
        .unwrap()
        .is_empty());
    // s2 should still exist
    assert!(store.get_symbol(&SymbolId("s2".into())).unwrap().is_some());
}

// ---- search_symbols ----

#[test]
fn search_symbols_substring_match() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("s1", "process_document", SymbolKind::Function, "a.py", 1, 5),
            sym("s2", "process_image", SymbolKind::Function, "a.py", 6, 10),
            sym("s3", "validate", SymbolKind::Function, "b.py", 1, 5),
        ])
        .unwrap();

    let results = store.search_symbols("process", None).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn search_symbols_exact_match_first() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("s1", "foo", SymbolKind::Function, "a.py", 1, 1),
            sym("s2", "foobar", SymbolKind::Function, "a.py", 2, 2),
        ])
        .unwrap();

    let results = store.search_symbols("foo", None).unwrap();
    assert_eq!(results[0].name, "foo");
}

#[test]
fn search_symbols_with_kind_filter() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("s1", "Foo", SymbolKind::Class, "a.py", 1, 10),
            sym("s2", "foo", SymbolKind::Function, "a.py", 11, 15),
        ])
        .unwrap();

    let results = store
        .search_symbols("foo", Some(SymbolKind::Function))
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].kind, SymbolKind::Function);
}

// ---- get_file_symbols ----

#[test]
fn get_file_symbols_returns_correct_file() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("s1", "a", SymbolKind::Function, "file1.py", 1, 5),
            sym("s2", "b", SymbolKind::Function, "file1.py", 6, 10),
            sym("s3", "c", SymbolKind::Function, "file2.py", 1, 5),
        ])
        .unwrap();

    let results = store.get_file_symbols(&PathBuf::from("file1.py")).unwrap();
    assert_eq!(results.len(), 2);
}

// ---- file records ----

#[test]
fn upsert_and_get_file_record() {
    let store = make_store();
    let record = FileRecord {
        path: PathBuf::from("src/main.py"),
        content_hash: "deadbeef".into(),
        last_indexed: 1000,
    };
    store.upsert_file_record(&record).unwrap();

    let got = store
        .get_file_record(&PathBuf::from("src/main.py"))
        .unwrap()
        .unwrap();
    assert_eq!(got.content_hash, "deadbeef");
    assert_eq!(got.last_indexed, 1000);
}

#[test]
fn file_record_upsert_updates() {
    let store = make_store();
    store
        .upsert_file_record(&FileRecord {
            path: PathBuf::from("a.py"),
            content_hash: "v1".into(),
            last_indexed: 1,
        })
        .unwrap();
    store
        .upsert_file_record(&FileRecord {
            path: PathBuf::from("a.py"),
            content_hash: "v2".into(),
            last_indexed: 2,
        })
        .unwrap();

    let got = store
        .get_file_record(&PathBuf::from("a.py"))
        .unwrap()
        .unwrap();
    assert_eq!(got.content_hash, "v2");
    assert_eq!(got.last_indexed, 2);
}

// ---- get_neighbors ----

#[test]
fn get_neighbors_depth() {
    // a -> b -> c -> d
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("a", "a", SymbolKind::Function, "f.py", 1, 1),
            sym("b", "b", SymbolKind::Function, "f.py", 2, 2),
            sym("c", "c", SymbolKind::Function, "f.py", 3, 3),
            sym("d", "d", SymbolKind::Function, "f.py", 4, 4),
        ])
        .unwrap();
    store
        .upsert_edges(&[
            edge("a", "b", EdgeKind::Calls),
            edge("b", "c", EdgeKind::Calls),
            edge("c", "d", EdgeKind::Calls),
        ])
        .unwrap();

    // depth 1: only b
    let sub = store
        .get_neighbors(&SymbolId("a".into()), Direction::Outgoing, 1)
        .unwrap();
    assert_eq!(sub.symbols.len(), 1);
    assert_eq!(sub.symbols[0].name, "b");

    // depth 2: b and c
    let sub = store
        .get_neighbors(&SymbolId("a".into()), Direction::Outgoing, 2)
        .unwrap();
    assert_eq!(sub.symbols.len(), 2);

    // depth 3: b, c, and d
    let sub = store
        .get_neighbors(&SymbolId("a".into()), Direction::Outgoing, 3)
        .unwrap();
    assert_eq!(sub.symbols.len(), 3);
}

#[test]
fn get_neighbors_incoming() {
    // a -> b, c -> b
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("a", "a", SymbolKind::Function, "f.py", 1, 1),
            sym("b", "b", SymbolKind::Function, "f.py", 2, 2),
            sym("c", "c", SymbolKind::Function, "f.py", 3, 3),
        ])
        .unwrap();
    store
        .upsert_edges(&[
            edge("a", "b", EdgeKind::Calls),
            edge("c", "b", EdgeKind::Calls),
        ])
        .unwrap();

    let sub = store
        .get_neighbors(&SymbolId("b".into()), Direction::Incoming, 1)
        .unwrap();
    assert_eq!(sub.symbols.len(), 2);
}

#[test]
fn get_neighbors_both() {
    // a -> b -> c
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("a", "a", SymbolKind::Function, "f.py", 1, 1),
            sym("b", "b", SymbolKind::Function, "f.py", 2, 2),
            sym("c", "c", SymbolKind::Function, "f.py", 3, 3),
        ])
        .unwrap();
    store
        .upsert_edges(&[
            edge("a", "b", EdgeKind::Calls),
            edge("b", "c", EdgeKind::Calls),
        ])
        .unwrap();

    let sub = store
        .get_neighbors(&SymbolId("b".into()), Direction::Both, 1)
        .unwrap();
    assert_eq!(sub.symbols.len(), 2); // a and c
}

// ---- get_shortest_path ----

#[test]
fn shortest_path_direct() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("a", "a", SymbolKind::Function, "f.py", 1, 1),
            sym("b", "b", SymbolKind::Function, "f.py", 2, 2),
        ])
        .unwrap();
    store
        .upsert_edges(&[edge("a", "b", EdgeKind::Calls)])
        .unwrap();

    let path = store
        .get_shortest_path(&SymbolId("a".into()), &SymbolId("b".into()))
        .unwrap()
        .unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].0, "a");
    assert_eq!(path[1].0, "b");
}

#[test]
fn shortest_path_multi_hop() {
    // a -> b -> c, a -> d -> c
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("a", "a", SymbolKind::Function, "f.py", 1, 1),
            sym("b", "b", SymbolKind::Function, "f.py", 2, 2),
            sym("c", "c", SymbolKind::Function, "f.py", 3, 3),
            sym("d", "d", SymbolKind::Function, "f.py", 4, 4),
        ])
        .unwrap();
    store
        .upsert_edges(&[
            edge("a", "b", EdgeKind::Calls),
            edge("b", "c", EdgeKind::Calls),
            edge("a", "d", EdgeKind::Calls),
            edge("d", "c", EdgeKind::Calls),
        ])
        .unwrap();

    let path = store
        .get_shortest_path(&SymbolId("a".into()), &SymbolId("c".into()))
        .unwrap()
        .unwrap();
    // Both paths are length 3 (a,b,c or a,d,c), either is valid
    assert_eq!(path.len(), 3);
    assert_eq!(path[0].0, "a");
    assert_eq!(path[2].0, "c");
}

#[test]
fn shortest_path_no_path() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("a", "a", SymbolKind::Function, "f.py", 1, 1),
            sym("b", "b", SymbolKind::Function, "f.py", 2, 2),
        ])
        .unwrap();
    // no edge between them

    let path = store
        .get_shortest_path(&SymbolId("a".into()), &SymbolId("b".into()))
        .unwrap();
    assert!(path.is_none());
}

// ---- Annotations ----

#[test]
fn annotations_crud() {
    let store = make_store();
    store
        .upsert_symbols(&[sym("s1", "foo", SymbolKind::Function, "a.py", 1, 5)])
        .unwrap();

    let ann = Annotation {
        symbol_id: SymbolId("s1".into()),
        source: "git_blame".into(),
        key: "last_author".into(),
        value: "alice".into(),
    };
    store.upsert_annotations(&[ann]).unwrap();

    let anns = store.get_annotations(&SymbolId("s1".into())).unwrap();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].value, "alice");

    // upsert updates
    store
        .upsert_annotations(&[Annotation {
            symbol_id: SymbolId("s1".into()),
            source: "git_blame".into(),
            key: "last_author".into(),
            value: "bob".into(),
        }])
        .unwrap();
    let anns = store.get_annotations(&SymbolId("s1".into())).unwrap();
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].value, "bob");

    // delete by source
    store
        .delete_annotations(&SymbolId("s1".into()), "git_blame")
        .unwrap();
    let anns = store.get_annotations(&SymbolId("s1".into())).unwrap();
    assert!(anns.is_empty());
}

// ---- Symbol with metadata ----

#[test]
fn symbol_with_metadata() {
    let store = make_store();
    let mut s = sym("s1", "foo", SymbolKind::Function, "a.py", 1, 5);
    s.metadata = Some(serde_json::json!({"async": true, "decorator": "route"}));
    store.upsert_symbols(&[s]).unwrap();

    let got = store.get_symbol(&SymbolId("s1".into())).unwrap().unwrap();
    let meta = got.metadata.unwrap();
    assert_eq!(meta["async"], true);
    assert_eq!(meta["decorator"], "route");
}

// ---- get_all_symbols / get_all_edges ----

#[test]
fn get_all_symbols_and_edges() {
    let store = make_store();
    store
        .upsert_symbols(&[
            sym("s1", "a", SymbolKind::Function, "a.py", 1, 5),
            sym("s2", "b", SymbolKind::Function, "b.py", 1, 5),
        ])
        .unwrap();
    store
        .upsert_edges(&[edge("s1", "s2", EdgeKind::Calls)])
        .unwrap();

    let all_sym = store.get_all_symbols().unwrap();
    assert_eq!(all_sym.len(), 2);

    let all_edg = store.get_all_edges().unwrap();
    assert_eq!(all_edg.len(), 1);
}

// ---- get_all_file_records ----

#[test]
fn get_all_file_records() {
    let store = make_store();
    store
        .upsert_file_record(&FileRecord {
            path: PathBuf::from("a.py"),
            content_hash: "h1".into(),
            last_indexed: 100,
        })
        .unwrap();
    store
        .upsert_file_record(&FileRecord {
            path: PathBuf::from("b.py"),
            content_hash: "h2".into(),
            last_indexed: 200,
        })
        .unwrap();

    let records = store.get_all_file_records().unwrap();
    assert_eq!(records.len(), 2);
}

// ---- RawEdge::resolve with language scoping ----

#[test]
fn resolve_edges_same_language() {
    use eagraph_core::RawEdge;
    use std::collections::HashMap;

    let symbols = vec![
        sym("py1", "Response", SymbolKind::Class, "models.py", 1, 10),
        sym("py2", "handler", SymbolKind::Function, "views.py", 1, 5),
        sym("ts1", "Response", SymbolKind::Class, "types.ts", 1, 10),
    ];

    let raw = vec![RawEdge {
        source: SymbolId("py2".into()),
        target_name: "Response".into(),
        kind: EdgeKind::Calls,
    }];

    let ext_to_lang: HashMap<String, String> = [("py", "python"), ("ts", "typescript")]
        .iter()
        .map(|(e, l)| (e.to_string(), l.to_string()))
        .collect();

    let resolved = RawEdge::resolve(&raw, &symbols, &ext_to_lang);
    assert_eq!(resolved.len(), 1);
    // Should resolve to Python Response, not TypeScript Response
    assert_eq!(resolved[0].target, SymbolId("py1".into()));
}

#[test]
fn resolve_edges_cross_language_dropped() {
    use eagraph_core::RawEdge;
    use std::collections::HashMap;

    let symbols = vec![
        sym("ts1", "Response", SymbolKind::Class, "types.ts", 1, 10),
        sym("py1", "handler", SymbolKind::Function, "views.py", 1, 5),
    ];

    // Python handler calls "Response" but only TS Response exists
    let raw = vec![RawEdge {
        source: SymbolId("py1".into()),
        target_name: "Response".into(),
        kind: EdgeKind::Calls,
    }];

    let ext_to_lang: HashMap<String, String> = [("py", "python"), ("ts", "typescript")]
        .iter()
        .map(|(e, l)| (e.to_string(), l.to_string()))
        .collect();

    let resolved = RawEdge::resolve(&raw, &symbols, &ext_to_lang);
    assert_eq!(resolved.len(), 0); // dropped — no Python Response exists
}
