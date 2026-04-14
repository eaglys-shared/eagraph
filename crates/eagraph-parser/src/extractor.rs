use std::ops::Range;
use std::path::Path;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language as TsLanguage, Node, Parser, Query, QueryCursor};

use eagraph_core::{Edge, EdgeKind, EagraphError, Symbol, SymbolKind};

use crate::symbol_id::make_symbol_id;
use crate::LanguageExtractor;

/// Language-specific configuration. This is all a new language needs to provide.
#[derive(Clone)]
pub struct LanguageConfig {
    pub name: String,
    pub ts_language: TsLanguage,
    pub queries: String,
    pub module_separator: String,
}

/// Generic tree-sitter extractor driven by .scm queries and LanguageConfig.
#[derive(Clone)]
pub struct GenericExtractor {
    config: LanguageConfig,
}

impl GenericExtractor {
    pub fn new(config: LanguageConfig) -> Self {
        Self { config }
    }
}

// Intermediate data extracted from query matches in a single pass.
struct RawClass {
    range: Range<usize>,
    name: String,
    sym: Symbol,
    base_names: Vec<String>,
}

struct RawFunc {
    range: Range<usize>,
    raw_name: String,
    line_start: u32,
    line_end: u32,
}

struct RawCall {
    byte_pos: usize,
    callee_name: String,
}

struct RawImport {
    name: String,
    line_start: u32,
    line_end: u32,
}

struct RawFromImport {
    module_name: String,
    imported_names: Vec<String>,
    byte_pos: usize,
    line_start: u32,
    line_end: u32,
}

impl LanguageExtractor for GenericExtractor {
    fn language_name(&self) -> &str {
        &self.config.name
    }

    fn extract(
        &self,
        file_path: &Path,
        source: &str,
    ) -> eagraph_core::Result<(Vec<Symbol>, Vec<Edge>)> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.config.ts_language)
            .map_err(|e| EagraphError::Parser(e.to_string()))?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| EagraphError::Parser("parse returned None".into()))?;

        let query = Query::new(&self.config.ts_language, &self.config.queries)
            .map_err(|e| EagraphError::Parser(format!("query compile: {:?}", e)))?;

        let src = source.as_bytes();
        let capture_names = query.capture_names();

        // Single pass: stream matches and extract raw data
        let mut classes = Vec::new();
        let mut funcs = Vec::new();
        let mut calls = Vec::new();
        let mut imports = Vec::new();
        let mut from_imports = Vec::new();

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), src);
        while let Some(m) = matches.next() {
            let prefix = match m.captures.first() {
                Some(c) => {
                    let name = capture_names[c.index as usize];
                    name.split('.').next().unwrap_or("")
                }
                None => continue,
            };

            match prefix {
                "class" => {
                    if let Some(raw) = extract_raw_class(&capture_names, m, src, file_path) {
                        classes.push(raw);
                    }
                }
                "func" => {
                    if let Some(raw) = extract_raw_func(&capture_names, m, src) {
                        funcs.push(raw);
                    }
                }
                "call" | "method_call" => {
                    if let Some(raw) = extract_raw_call(&capture_names, m, src, prefix) {
                        calls.push(raw);
                    }
                }
                "import" => {
                    if let Some(raw) = extract_raw_import(&capture_names, m, src) {
                        imports.push(raw);
                    }
                }
                "from_import" => {
                    if let Some(raw) = extract_raw_from_import(&capture_names, m, src) {
                        from_imports.push(raw);
                    }
                }
                _ => {}
            }
        }
        drop(matches);

        // Now build symbols and edges from raw data
        let mut symbols = Vec::new();
        let mut edges = Vec::new();

        // Sort classes by start for scope lookup
        classes.sort_by_key(|c| c.range.start);

        // Process classes
        for c in &classes {
            symbols.push(c.sym.clone());
            for base in &c.base_names {
                let base_id = make_symbol_id(file_path, base, "class");
                edges.push(Edge {
                    source: c.sym.id.clone(),
                    target: base_id,
                    kind: EdgeKind::Inherits,
                });
            }
        }

        // Process functions — determine method vs function from class containment
        let mut func_ranges: Vec<(Range<usize>, eagraph_core::SymbolId)> = Vec::new();
        for f in &funcs {
            let enclosing_class = find_enclosing_name(&classes, f.range.start);
            let (kind, kind_str, name) = match enclosing_class {
                Some(class_name) => {
                    let qualified = format!("{}.{}", class_name, f.raw_name);
                    (SymbolKind::Method, "method", qualified)
                }
                None => (SymbolKind::Function, "function", f.raw_name.clone()),
            };

            let id = make_symbol_id(file_path, &name, kind_str);
            symbols.push(Symbol {
                id: id.clone(),
                name,
                kind,
                file_path: file_path.to_path_buf(),
                line_start: f.line_start,
                line_end: f.line_end,
                metadata: None,
            });
            func_ranges.push((f.range.clone(), id));
        }
        func_ranges.sort_by_key(|(r, _)| r.start);

        let file_scope_id =
            make_symbol_id(file_path, file_path.to_str().unwrap_or(""), "module");

        // Process calls
        for c in &calls {
            let caller_id = find_enclosing_id(&func_ranges, c.byte_pos)
                .unwrap_or_else(|| file_scope_id.clone());
            let callee_id = make_symbol_id(file_path, &c.callee_name, "function");
            edges.push(Edge {
                source: caller_id,
                target: callee_id,
                kind: EdgeKind::Calls,
            });
        }

        // Process imports
        for imp in &imports {
            let id = make_symbol_id(file_path, &imp.name, "module");
            symbols.push(Symbol {
                id,
                name: imp.name.clone(),
                kind: SymbolKind::Module,
                file_path: file_path.to_path_buf(),
                line_start: imp.line_start,
                line_end: imp.line_end,
                metadata: None,
            });
        }

        // Process from-imports
        let sep = &self.config.module_separator;
        for fi in &from_imports {
            let scope_id = find_enclosing_id(&func_ranges, fi.byte_pos)
                .unwrap_or_else(|| file_scope_id.clone());

            for imported in &fi.imported_names {
                let full_path = if fi.module_name.is_empty() {
                    imported.clone()
                } else {
                    format!("{}{}{}", fi.module_name, sep, imported)
                };
                let target_id = make_symbol_id(file_path, &full_path, "module");
                edges.push(Edge {
                    source: scope_id.clone(),
                    target: target_id.clone(),
                    kind: EdgeKind::Imports,
                });
                symbols.push(Symbol {
                    id: target_id,
                    name: full_path,
                    kind: SymbolKind::Module,
                    file_path: file_path.to_path_buf(),
                    line_start: fi.line_start,
                    line_end: fi.line_end,
                    metadata: None,
                });
            }
        }

        Ok((symbols, edges))
    }
}

// --- Single-pass extractors: query match → raw data ---

fn get_capture<'a>(
    names: &[&str],
    m: &tree_sitter::QueryMatch<'a, 'a>,
    capture_name: &str,
) -> Option<Node<'a>> {
    m.captures
        .iter()
        .find(|c| names[c.index as usize] == capture_name)
        .map(|c| c.node)
}

fn extract_raw_class(
    names: &[&str],
    m: &tree_sitter::QueryMatch,
    src: &[u8],
    file_path: &Path,
) -> Option<RawClass> {
    let def = get_capture(names, m, "class.def")?;
    let name_node = get_capture(names, m, "class.name")?;
    let name = name_node.utf8_text(src).ok()?.to_string();
    let id = make_symbol_id(file_path, &name, "class");

    let mut base_names = Vec::new();
    if let Some(bases) = get_capture(names, m, "class.bases") {
        let mut cursor = bases.walk();
        for child in bases.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Ok(base) = child.utf8_text(src) {
                    base_names.push(base.to_string());
                }
            }
        }
    }

    Some(RawClass {
        range: def.byte_range(),
        name: name.clone(),
        sym: Symbol {
            id,
            name,
            kind: SymbolKind::Class,
            file_path: file_path.to_path_buf(),
            line_start: def.start_position().row as u32 + 1,
            line_end: def.end_position().row as u32 + 1,
            metadata: None,
        },
        base_names,
    })
}

fn extract_raw_func(
    names: &[&str],
    m: &tree_sitter::QueryMatch,
    src: &[u8],
) -> Option<RawFunc> {
    let def = get_capture(names, m, "func.def")?;
    let name_node = get_capture(names, m, "func.name")?;
    let raw_name = name_node.utf8_text(src).ok()?.to_string();

    Some(RawFunc {
        range: def.byte_range(),
        raw_name,
        line_start: def.start_position().row as u32 + 1,
        line_end: def.end_position().row as u32 + 1,
    })
}

fn extract_raw_call(
    names: &[&str],
    m: &tree_sitter::QueryMatch,
    src: &[u8],
    prefix: &str,
) -> Option<RawCall> {
    let name_capture = format!("{}.name", prefix);
    let def_capture = format!("{}.def", prefix);
    let name_node = get_capture(names, m, &name_capture)?;
    let def_node = get_capture(names, m, &def_capture);
    let callee = name_node.utf8_text(src).ok()?.to_string();
    if callee.is_empty() {
        return None;
    }
    Some(RawCall {
        byte_pos: def_node.map(|n| n.start_byte()).unwrap_or(0),
        callee_name: callee,
    })
}

fn extract_raw_import(
    names: &[&str],
    m: &tree_sitter::QueryMatch,
    src: &[u8],
) -> Option<RawImport> {
    let module_node = get_capture(names, m, "import.module")?;
    let name = module_node.utf8_text(src).ok()?.to_string();
    Some(RawImport {
        name,
        line_start: module_node.start_position().row as u32 + 1,
        line_end: module_node.end_position().row as u32 + 1,
    })
}

fn extract_raw_from_import(
    names: &[&str],
    m: &tree_sitter::QueryMatch,
    src: &[u8],
) -> Option<RawFromImport> {
    let def = get_capture(names, m, "from_import.def")?;
    let module_name = get_capture(names, m, "from_import.module")
        .and_then(|n| n.utf8_text(src).ok())
        .unwrap_or("")
        .to_string();

    // Collect imported names from the statement's children
    let module_name_node = def.child_by_field_name("module_name");
    let mut imported_names = Vec::new();
    let mut cursor = def.walk();
    for child in def.children(&mut cursor) {
        let is_module = module_name_node.map_or(false, |mn| child.id() == mn.id());
        if is_module {
            continue;
        }
        match child.kind() {
            "dotted_name" | "identifier" => {
                if let Ok(name) = child.utf8_text(src) {
                    imported_names.push(name.to_string());
                }
            }
            _ => {}
        }
    }

    if imported_names.is_empty() {
        return None;
    }

    Some(RawFromImport {
        module_name,
        imported_names,
        byte_pos: def.start_byte(),
        line_start: def.start_position().row as u32 + 1,
        line_end: def.end_position().row as u32 + 1,
    })
}

// --- Scope lookup helpers ---

fn find_enclosing_name(classes: &[RawClass], byte_pos: usize) -> Option<&str> {
    classes
        .iter()
        .rev()
        .find(|c| c.range.start <= byte_pos && byte_pos < c.range.end)
        .map(|c| c.name.as_str())
}

fn find_enclosing_id(
    ranges: &[(Range<usize>, eagraph_core::SymbolId)],
    byte_pos: usize,
) -> Option<eagraph_core::SymbolId> {
    ranges
        .iter()
        .rev()
        .find(|(r, _)| r.start <= byte_pos && byte_pos < r.end)
        .map(|(_, id)| id.clone())
}
