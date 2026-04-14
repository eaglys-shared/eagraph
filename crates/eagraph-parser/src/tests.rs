#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use eagraph_core::{EdgeKind, SymbolKind};

    use crate::LanguageExtractor;

    fn test_grammars_dir() -> PathBuf {
        PathBuf::from(env!("OUT_DIR")).join("test_grammars")
    }

    fn parse(source: &str) -> (Vec<eagraph_core::Symbol>, Vec<eagraph_core::RawEdge>) {
        let grammars_dir = test_grammars_dir();
        assert!(
            grammars_dir.join("python.toml").exists(),
            "test grammars not built. Dir: {}",
            grammars_dir.display()
        );
        let registry = crate::LanguageRegistry::from_dir(&grammars_dir).unwrap();
        let extractor = registry
            .extractor_for(Path::new("test.py"))
            .expect("python extractor not loaded from test grammars");
        extractor.extract(Path::new("test.py"), source).unwrap()
    }

    fn find_symbol<'a>(
        symbols: &'a [eagraph_core::Symbol],
        name: &str,
        kind: SymbolKind,
    ) -> Option<&'a eagraph_core::Symbol> {
        symbols.iter().find(|s| s.name == name && s.kind == kind)
    }

    fn has_edge(
        edges: &[eagraph_core::RawEdge],
        source_name: &str,
        target_name: &str,
        kind: EdgeKind,
        symbols: &[eagraph_core::Symbol],
    ) -> bool {
        edges.iter().any(|e| {
            let src = symbols.iter().find(|s| s.id == e.source);
            match src {
                Some(s) => s.name == source_name && e.target_name == target_name && e.kind == kind,
                None => false,
            }
        })
    }

    // ---- Function definitions ----

    #[test]
    fn extracts_function_defs() {
        let (syms, _) = parse(
            r#"
def foo():
    pass

def bar(x, y):
    return x + y
"#,
        );
        assert!(find_symbol(&syms, "foo", SymbolKind::Function).is_some());
        assert!(find_symbol(&syms, "bar", SymbolKind::Function).is_some());
    }

    #[test]
    fn function_line_range() {
        let (syms, _) = parse(
            r#"def hello():
    print("hello")
    print("world")
"#,
        );
        let f = find_symbol(&syms, "hello", SymbolKind::Function).unwrap();
        assert_eq!(f.line_start, 1);
        assert_eq!(f.line_end, 3);
    }

    // ---- Class definitions ----

    #[test]
    fn extracts_class_def() {
        let (syms, _) = parse(
            r#"
class MyClass:
    def method_a(self):
        pass

    def method_b(self):
        pass
"#,
        );
        assert!(find_symbol(&syms, "MyClass", SymbolKind::Class).is_some());
        assert!(find_symbol(&syms, "MyClass.method_a", SymbolKind::Method).is_some());
        assert!(find_symbol(&syms, "MyClass.method_b", SymbolKind::Method).is_some());
    }

    // ---- Inheritance ----

    #[test]
    fn extracts_inheritance() {
        let (syms, edges) = parse(
            r#"
class Base:
    pass

class Child(Base):
    pass
"#,
        );
        assert!(find_symbol(&syms, "Base", SymbolKind::Class).is_some());
        assert!(find_symbol(&syms, "Child", SymbolKind::Class).is_some());
        assert!(has_edge(&edges, "Child", "Base", EdgeKind::Inherits, &syms));
    }

    // ---- Function calls ----

    #[test]
    fn extracts_function_calls() {
        let (syms, edges) = parse(
            r#"
def caller():
    callee()

def callee():
    pass
"#,
        );
        assert!(find_symbol(&syms, "caller", SymbolKind::Function).is_some());
        assert!(find_symbol(&syms, "callee", SymbolKind::Function).is_some());
        assert!(has_edge(&edges, "caller", "callee", EdgeKind::Calls, &syms));
    }

    #[test]
    fn extracts_nested_calls() {
        let (syms, edges) = parse(
            r#"
def outer():
    inner(helper())

def inner(x):
    pass

def helper():
    pass
"#,
        );
        assert!(has_edge(&edges, "outer", "inner", EdgeKind::Calls, &syms));
        assert!(has_edge(&edges, "outer", "helper", EdgeKind::Calls, &syms));
    }

    // ---- Imports ----

    #[test]
    fn extracts_from_import() {
        let (syms, edges) = parse(
            r#"
from os.path import join
"#,
        );
        let import_sym = syms
            .iter()
            .find(|s| s.name == "os.path.join" && s.kind == SymbolKind::Module);
        assert!(import_sym.is_some(), "expected os.path.join module symbol, got: {:?}", syms.iter().map(|s| &s.name).collect::<Vec<_>>());

        let has_import = edges.iter().any(|e| e.kind == EdgeKind::Imports);
        assert!(has_import);
    }

    #[test]
    fn extracts_multi_from_import() {
        let (syms, _edges) = parse(
            r#"
from typing import List, Dict, Optional
"#,
        );
        let imports: Vec<&str> = syms
            .iter()
            .filter(|s| s.kind == SymbolKind::Module && s.name.starts_with("typing."))
            .map(|s| s.name.as_str())
            .collect();
        assert!(imports.contains(&"typing.List"), "got: {:?}", imports);
        assert!(imports.contains(&"typing.Dict"), "got: {:?}", imports);
        assert!(imports.contains(&"typing.Optional"), "got: {:?}", imports);
    }

    // ---- Registry ----

    #[test]
    fn registry_loads_from_dir() {
        let registry = crate::LanguageRegistry::from_dir(&test_grammars_dir()).unwrap();
        assert!(registry.supports(Path::new("test.py")));
        assert!(registry.supports(Path::new("test.pyi")));
        assert!(!registry.supports(Path::new("test.rb")));
    }

    #[test]
    fn registry_from_missing_dir_is_empty() {
        let registry = crate::LanguageRegistry::from_dir(Path::new("/nonexistent")).unwrap();
        assert!(registry.supported_extensions().is_empty());
    }
}
