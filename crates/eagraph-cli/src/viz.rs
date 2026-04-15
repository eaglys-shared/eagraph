use std::collections::HashMap;

use anyhow::Result;
use tiny_http::{Header, Response, Server};

use eagraph_core::{Edge, Symbol};

const INDEX_HTML: &str = include_str!("../web/index.html");
const STYLE_CSS: &str = include_str!("../web/style.css");
const APP_JS: &str = include_str!("../web/app.js");
const D3_JS: &str = include_str!("../web/d3.v7.min.js");

pub fn serve(repos: Vec<(String, Vec<Symbol>, Vec<Edge>)>, port: u16) -> Result<()> {
    // Pre-build JSON for each repo
    let mut repo_data: HashMap<String, String> = HashMap::new();
    let mut repo_names: Vec<String> = Vec::new();
    for (name, symbols, edges) in &repos {
        repo_data.insert(name.clone(), build_graph_json(symbols, edges));
        repo_names.push(name.clone());
    }
    let repos_json = serde_json::to_string(&repo_names)?;

    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("failed to bind {}: {}", addr, e))?;

    println!("http://localhost:{}", port);

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let response = if url == "/" {
            Response::from_string(INDEX_HTML).with_header(ct("text/html"))
        } else if url == "/style.css" {
            Response::from_string(STYLE_CSS).with_header(ct("text/css"))
        } else if url == "/app.js" {
            Response::from_string(APP_JS).with_header(ct("application/javascript"))
        } else if url == "/d3.js" {
            Response::from_string(D3_JS).with_header(ct("application/javascript"))
        } else if url == "/repos.json" {
            Response::from_string(&repos_json).with_header(ct("application/json"))
        } else if url.starts_with("/data/") {
            let repo_name = &url[6..]; // strip "/data/"
            match repo_data.get(repo_name) {
                Some(data) => Response::from_string(data.as_str()).with_header(ct("application/json")),
                None => Response::from_string("404").with_status_code(404).with_header(ct("text/plain")),
            }
        } else {
            Response::from_string("404").with_status_code(404).with_header(ct("text/plain"))
        };
        let _ = request.respond(response);
    }

    Ok(())
}

fn ct(mime: &str) -> Header {
    Header::from_bytes("Content-Type", mime).expect("valid header")
}

fn build_graph_json(symbols: &[Symbol], edges: &[Edge]) -> String {
    let sym_nodes: Vec<serde_json::Value> = symbols
        .iter()
        .map(|s| {
            let lang = s.file_path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            serde_json::json!({
                "id": s.id.0,
                "name": s.name,
                "kind": s.kind.to_string(),
                "file": s.file_path.to_str().unwrap_or(""),
                "lang": lang,
                "lineStart": s.line_start,
                "lineEnd": s.line_end,
            })
        })
        .collect();

    let id_set: std::collections::HashSet<&str> =
        symbols.iter().map(|s| s.id.0.as_str()).collect();

    let sym_links: Vec<serde_json::Value> = edges
        .iter()
        .filter(|e| id_set.contains(e.source.0.as_str()) && id_set.contains(e.target.0.as_str()))
        .map(|e| {
            serde_json::json!({
                "source": e.source.0,
                "target": e.target.0,
                "kind": e.kind.to_string(),
            })
        })
        .collect();

    let mut symbol_to_file: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut file_symbol_count: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for s in symbols {
        let file = s.file_path.to_str().unwrap_or("");
        symbol_to_file.insert(&s.id.0, file);
        *file_symbol_count.entry(file).or_insert(0) += 1;
    }

    let file_nodes: Vec<serde_json::Value> = file_symbol_count
        .iter()
        .map(|(file, count)| {
            let p = std::path::Path::new(file);
            let short = p.file_name().and_then(|f| f.to_str()).unwrap_or(file);
            let lang = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            serde_json::json!({
                "id": file, "name": short, "kind": "file", "file": file, "lang": lang, "symbols": count,
            })
        })
        .collect();

    let mut file_edge_counts: std::collections::HashMap<(&str, &str, String), usize> =
        std::collections::HashMap::new();
    for e in edges {
        if let (Some(src_file), Some(tgt_file)) = (
            symbol_to_file.get(e.source.0.as_str()),
            symbol_to_file.get(e.target.0.as_str()),
        ) {
            if src_file != tgt_file {
                *file_edge_counts.entry((src_file, tgt_file, e.kind.to_string())).or_insert(0) += 1;
            }
        }
    }

    let file_links: Vec<serde_json::Value> = file_edge_counts
        .iter()
        .map(|((src, tgt, kind), count)| {
            serde_json::json!({ "source": src, "target": tgt, "kind": kind, "weight": count })
        })
        .collect();

    serde_json::json!({
        "symbols": { "nodes": sym_nodes, "links": sym_links },
        "files": { "nodes": file_nodes, "links": file_links },
    })
    .to_string()
}
