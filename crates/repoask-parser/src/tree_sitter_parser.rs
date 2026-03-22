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
    parser
        .set_language(language)
        .expect("Failed to set tree-sitter language");

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

/// Extract parameter names from a parameters/parameter_list node.
fn extract_param_names(node: Node, source: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        // Look for identifier-like children in parameter nodes
        if let Some(name_node) = find_param_name(child, source) {
            params.push(name_node);
        }
    }

    params
}

fn find_param_name(node: Node, source: &str) -> Option<String> {
    let kind = node.kind();

    // Direct identifier
    if kind == "identifier" || kind == "name" || kind == "shorthand_field_identifier" {
        return Some(source[node.byte_range()].to_string());
    }

    // Parameter-like nodes: look for the first identifier child
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

/// Try to extract a doc comment from the node's preceding siblings.
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

// ---------------------------------------------------------------------------
// Tree-sitter query definitions per language
// ---------------------------------------------------------------------------

pub fn rust_query() -> &'static str {
    r#"
(function_item
  name: (identifier) @name
  parameters: (parameters) @params) @definition.function

(struct_item
  name: (type_identifier) @name) @definition.struct

(enum_item
  name: (type_identifier) @name) @definition.enum

(trait_item
  name: (type_identifier) @name) @definition.trait

(type_item
  name: (type_identifier) @name) @definition.type

(const_item
  name: (identifier) @name) @definition.const

(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name
      parameters: (parameters) @params) @definition.method))
"#
}

pub fn python_query() -> &'static str {
    r#"
(function_definition
  name: (identifier) @name
  parameters: (parameters) @params) @definition.function

(class_definition
  name: (identifier) @name) @definition.class
"#
}

pub fn go_query() -> &'static str {
    r#"
(function_declaration
  name: (identifier) @name
  parameters: (parameter_list) @params) @definition.function

(method_declaration
  name: (field_identifier) @name
  parameters: (parameter_list) @params) @definition.method

(type_declaration
  (type_spec
    name: (type_identifier) @name) @definition.type)
"#
}

pub fn java_query() -> &'static str {
    r#"
(method_declaration
  name: (identifier) @name
  parameters: (formal_parameters) @params) @definition.method

(class_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(enum_declaration
  name: (identifier) @name) @definition.enum
"#
}

pub fn c_query() -> &'static str {
    r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name
    parameters: (parameter_list) @params)) @definition.function

(struct_specifier
  name: (type_identifier) @name) @definition.struct

(enum_specifier
  name: (type_identifier) @name) @definition.enum
"#
}

pub fn cpp_query() -> &'static str {
    r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name
    parameters: (parameter_list) @params)) @definition.function

(class_specifier
  name: (type_identifier) @name) @definition.class

(struct_specifier
  name: (type_identifier) @name) @definition.struct

(enum_specifier
  name: (type_identifier) @name) @definition.enum
"#
}

pub fn ruby_query() -> &'static str {
    r#"
(method
  name: (identifier) @name
  parameters: (method_parameters) @params) @definition.method

(class
  name: (constant) @name) @definition.class

(module
  name: (constant) @name) @definition.class
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_extraction() {
        let source = r#"
/// Add two numbers
fn add(a: i32, b: i32) -> i32 {
    a + b
}

struct Point {
    x: f64,
    y: f64,
}

enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let lang: Language = tree_sitter_rust::LANGUAGE.into();
        let symbols = extract_symbols(source, "lib.rs", &lang, rust_query());
        assert!(symbols.iter().any(|s| s.name == "add" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == SymbolKind::Struct));
        assert!(symbols.iter().any(|s| s.name == "Color" && s.kind == SymbolKind::Enum));
        // Doc comment
        let add = symbols.iter().find(|s| s.name == "add").unwrap();
        assert!(add.doc_comment.as_ref().unwrap().contains("Add two numbers"));
    }

    #[test]
    fn test_python_extraction() {
        let source = r#"
def greet(name):
    return f"Hello, {name}"

class UserService:
    def find(self, user_id):
        pass
"#;
        let lang: Language = tree_sitter_python::LANGUAGE.into();
        let symbols = extract_symbols(source, "app.py", &lang, python_query());
        assert!(symbols.iter().any(|s| s.name == "greet" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        // Methods inside classes are also functions in Python grammar
        assert!(symbols.iter().any(|s| s.name == "find"));
    }

    #[test]
    fn test_go_extraction() {
        let source = r#"
func main() {
    fmt.Println("hello")
}

type Config struct {
    Port int
}
"#;
        let lang: Language = tree_sitter_go::LANGUAGE.into();
        let symbols = extract_symbols(source, "main.go", &lang, go_query());
        assert!(symbols.iter().any(|s| s.name == "main" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "Config" && s.kind == SymbolKind::Type));
    }
}
