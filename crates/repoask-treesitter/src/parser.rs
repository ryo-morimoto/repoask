//! Core tree-sitter symbol extraction logic.

use repoask_core::types::{Symbol, SymbolKind};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};

/// Extract symbols from source code using tree-sitter with a language-specific query.
pub fn extract_symbols(
    source: &str,
    filepath: &str,
    language: &Language,
    query_source: &str,
) -> Vec<Symbol> {
    let mut parser = Parser::new();
    if parser.set_language(language).is_err() {
        return vec![];
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return vec![],
    };

    let query = match Query::new(language, query_source) {
        Ok(q) => q,
        Err(_) => return vec![],
    };

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    let capture_names = query.capture_names();

    let mut symbols = Vec::new();

    while let Some(m) = matches.next() {
        let mut name = String::new();
        let mut kind = SymbolKind::Function;
        let mut start_line = 0u32;
        let mut end_line = 0u32;
        let mut params = Vec::new();
        let mut def_node: Option<Node> = None;

        for capture in m.captures {
            let capture_name = capture_names[capture.index as usize];
            let node = capture.node;
            let text = &source[node.byte_range()];

            match capture_name {
                "name" => name = text.to_string(),
                "params" => {
                    params = extract_param_names(node, source);
                }
                _ if capture_name.starts_with("definition.") => {
                    kind = match capture_name {
                        "definition.function" => SymbolKind::Function,
                        "definition.method" => SymbolKind::Method,
                        "definition.class" => SymbolKind::Class,
                        "definition.struct" => SymbolKind::Struct,
                        "definition.enum" => SymbolKind::Enum,
                        "definition.interface" => SymbolKind::Interface,
                        "definition.type" => SymbolKind::Type,
                        "definition.trait" => SymbolKind::Trait,
                        "definition.const" => SymbolKind::Const,
                        _ => SymbolKind::Function,
                    };
                    start_line = node.start_position().row as u32 + 1;
                    end_line = node.end_position().row as u32 + 1;
                    def_node = Some(node);
                }
                _ => {}
            }
        }

        if !name.is_empty() && start_line > 0 {
            let doc_comment = def_node.and_then(|n| extract_doc_comment(n, source));
            symbols.push(Symbol {
                name,
                kind,
                filepath: filepath.to_string(),
                start_line,
                end_line,
                doc_comment,
                params,
            });
        }
    }

    symbols
}

fn extract_param_names(node: Node, source: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if let Some(name_node) = find_param_name(child, source) {
            params.push(name_node);
        }
    }

    params
}

fn find_param_name(node: Node, source: &str) -> Option<String> {
    let kind = node.kind();

    if kind == "identifier" || kind == "name" || kind == "shorthand_field_identifier" {
        return Some(source[node.byte_range()].to_string());
    }

    if kind.contains("parameter") || kind == "typed_parameter" || kind == "pair" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return Some(source[child.byte_range()].to_string());
            }
        }
    }

    None
}

fn extract_doc_comment(node: Node, source: &str) -> Option<String> {
    let mut current = node;
    let mut comment_lines = Vec::new();

    while let Some(prev) = current.prev_sibling() {
        let kind = prev.kind();
        if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
            let text = source[prev.byte_range()].trim().to_string();
            comment_lines.push(text);
            current = prev;
        } else {
            break;
        }
    }

    if comment_lines.is_empty() {
        return None;
    }

    comment_lines.reverse();
    let joined = comment_lines
        .iter()
        .map(|c| {
            c.trim_start_matches("//")
                .trim_start_matches("///")
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim_start_matches('#')
                .trim_start_matches('*')
                .trim()
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}
