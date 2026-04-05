//! Core tree-sitter symbol extraction logic.

#![allow(
    clippy::redundant_pub_crate,
    reason = "crate-private parser helpers are consumed from the root module"
)]

use std::cell::RefCell;

use repoask_core::types::{
    CommentInfo, CommentSource, ExportInfo, ExportKind, Publicness, Symbol, SymbolKind,
};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator};

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// Extract symbols from source code using tree-sitter with a language-specific query.
pub(crate) fn extract_symbols(
    source: &str,
    filepath: &str,
    language: &Language,
    query_source: &str,
) -> Vec<Symbol> {
    let comment_source = comment_source_for_filepath(filepath);
    let tree = PARSER.with(|p| {
        let mut parser = p.borrow_mut();
        if parser.set_language(language).is_err() {
            return None;
        }
        parser.parse(source, None)
    });

    let Some(tree) = tree else { return vec![] };

    let Ok(query) = Query::new(language, query_source) else {
        return vec![];
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
                "name" => text.clone_into(&mut name),
                "params" => {
                    params = extract_param_names(node, source);
                }
                _ if capture_name.starts_with("definition.") => {
                    kind = match capture_name {
                        "definition.method" => SymbolKind::Method,
                        "definition.class" => SymbolKind::Class,
                        "definition.module" => SymbolKind::Module,
                        "definition.struct" => SymbolKind::Struct,
                        "definition.enum" => SymbolKind::Enum,
                        "definition.interface" => SymbolKind::Interface,
                        "definition.type" => SymbolKind::Type,
                        "definition.trait" => SymbolKind::Trait,
                        "definition.const" => SymbolKind::Const,
                        _ => SymbolKind::Function,
                    };
                    start_line = line_number_1based(node.start_position().row);
                    end_line = line_number_1based(node.end_position().row);
                    def_node = Some(node);
                }
                _ => {}
            }
        }

        if let Some(node) = def_node {
            kind = refine_symbol_kind(kind, node, filepath);
        }

        if !name.is_empty() && start_line > 0 {
            let comment =
                def_node.and_then(|node| extract_doc_comment(node, source, comment_source));
            let export = def_node.map_or_else(ExportInfo::unknown, |node| {
                extract_export_info(node, filepath, kind, &name, source)
            });
            symbols.push(Symbol {
                signature_preview: Some(build_signature_preview(kind, &name, &params)),
                comment,
                export,
                name,
                kind,
                filepath: filepath.to_owned(),
                start_line,
                end_line,
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
        return Some(source[node.byte_range()].to_owned());
    }

    if kind.contains("parameter") || kind == "typed_parameter" || kind == "pair" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "name" {
                return Some(source[child.byte_range()].to_owned());
            }
        }
    }

    None
}

fn extract_doc_comment(
    node: Node,
    source: &str,
    comment_source: CommentSource,
) -> Option<CommentInfo> {
    let mut current = node;
    let mut comment_lines = Vec::new();

    while let Some(prev) = current.prev_sibling() {
        let kind = prev.kind();
        if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
            let text = source[prev.byte_range()].trim().to_owned();
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
        CommentInfo::from_normalized_text(&joined, comment_source)
    }
}

fn extract_export_info(
    node: Node,
    filepath: &str,
    kind: SymbolKind,
    name: &str,
    source: &str,
) -> ExportInfo {
    match extension_for_filepath(filepath) {
        Some("rs") => rust_export_info(node, kind, source),
        Some("py" | "pyi") => python_export_info(kind, name),
        Some("go") => go_export_info(kind, name),
        Some("java") => java_export_info(node, kind, source),
        Some("rb") => ruby_export_info(kind, name),
        Some("c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx") => {
            c_like_export_info(node, kind, source)
        }
        _ => ExportInfo::unknown(),
    }
}

fn refine_symbol_kind(kind: SymbolKind, node: Node, filepath: &str) -> SymbolKind {
    match extension_for_filepath(filepath) {
        Some("py" | "pyi")
            if kind == SymbolKind::Function && has_ancestor_kind(node, "class_definition") =>
        {
            SymbolKind::Method
        }
        _ => kind,
    }
}

fn has_ancestor_kind(mut node: Node, target_kind: &str) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == target_kind {
            return true;
        }
        node = parent;
    }
    false
}

fn rust_export_info(node: Node, kind: SymbolKind, source: &str) -> ExportInfo {
    let export_kind = if kind == SymbolKind::Method {
        ExportKind::ModuleMember
    } else {
        ExportKind::Named
    };

    let item_text = source[node.byte_range()].trim_start();
    if item_text.starts_with("pub ") {
        ExportInfo::new(Publicness::Public, export_kind)
    } else if item_text.starts_with("pub(") {
        ExportInfo::new(Publicness::Package, export_kind)
    } else if kind == SymbolKind::Method {
        ExportInfo::new(Publicness::Private, ExportKind::ModuleMember)
    } else {
        ExportInfo::private()
    }
}

fn python_export_info(kind: SymbolKind, name: &str) -> ExportInfo {
    visibility_by_convention(kind, name)
}

fn go_export_info(kind: SymbolKind, name: &str) -> ExportInfo {
    visibility_by_convention(kind, name)
}

fn ruby_export_info(kind: SymbolKind, name: &str) -> ExportInfo {
    visibility_by_convention(kind, name)
}

fn java_export_info(node: Node, kind: SymbolKind, source: &str) -> ExportInfo {
    let export_kind = member_aware_export_kind(kind);
    let item_text = source[node.byte_range()].trim_start();
    if item_text.starts_with("public ") {
        ExportInfo::new(Publicness::Public, export_kind)
    } else if item_text.starts_with("private ") {
        ExportInfo::new(Publicness::Private, export_kind)
    } else {
        ExportInfo::new(Publicness::Package, export_kind)
    }
}

fn c_like_export_info(node: Node, kind: SymbolKind, source: &str) -> ExportInfo {
    let item_text = source[node.byte_range()].trim_start();
    if item_text.starts_with("static ") {
        ExportInfo::new(Publicness::Private, member_aware_export_kind(kind))
    } else {
        ExportInfo::new(Publicness::Public, member_aware_export_kind(kind))
    }
}

fn visibility_by_convention(kind: SymbolKind, name: &str) -> ExportInfo {
    if name.starts_with('_') {
        ExportInfo::new(Publicness::Private, member_aware_export_kind(kind))
    } else if name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_uppercase() || ch.is_lowercase())
    {
        ExportInfo::new(Publicness::Public, member_aware_export_kind(kind))
    } else {
        ExportInfo::unknown()
    }
}

fn member_aware_export_kind(kind: SymbolKind) -> ExportKind {
    if kind == SymbolKind::Method {
        ExportKind::ModuleMember
    } else {
        ExportKind::Named
    }
}

fn comment_source_for_filepath(filepath: &str) -> CommentSource {
    match extension_for_filepath(filepath) {
        Some("rs") => CommentSource::RustDoc,
        _ => CommentSource::PlainComment,
    }
}

fn extension_for_filepath(filepath: &str) -> Option<&str> {
    filepath.rsplit('.').next()
}

fn build_signature_preview(kind: SymbolKind, name: &str, params: &[String]) -> String {
    match kind {
        SymbolKind::Function | SymbolKind::Method => format!("{name}({})", params.join(", ")),
        _ => name.to_owned(),
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn line_number_1based(value: usize) -> u32 {
    saturating_u32(value).saturating_add(1)
}
