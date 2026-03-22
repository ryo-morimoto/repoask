use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use repoask_core::types::{Symbol, SymbolKind};

/// Extract symbols from TypeScript/JavaScript source code using oxc_parser.
pub fn extract_ts_symbols(source: &str, filepath: &str) -> Vec<Symbol> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filepath).unwrap_or_default();

    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        return vec![];
    }

    let line_index = LineIndex::new(source);
    let mut symbols = Vec::new();

    for stmt in &ret.program.body {
        extract_from_statement(stmt, filepath, &line_index, source, &mut symbols);
    }

    symbols
}

fn extract_from_statement(
    stmt: &Statement<'_>,
    filepath: &str,
    line_index: &LineIndex,
    source: &str,
    symbols: &mut Vec<Symbol>,
) {
    match stmt {
        Statement::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                symbols.push(Symbol {
                    name: id.name.to_string(),
                    kind: SymbolKind::Function,
                    filepath: filepath.to_string(),
                    start_line: line_index.line_of(func.span.start),
                    end_line: line_index.line_of(func.span.end),
                    doc_comment: extract_leading_comment(source, func.span.start),
                    params: extract_params(&func.params),
                });
            }
        }
        Statement::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                let class_name = id.name.to_string();
                symbols.push(Symbol {
                    name: class_name.clone(),
                    kind: SymbolKind::Class,
                    filepath: filepath.to_string(),
                    start_line: line_index.line_of(class.span.start),
                    end_line: line_index.line_of(class.span.end),
                    doc_comment: extract_leading_comment(source, class.span.start),
                    params: vec![],
                });
                // Extract methods
                extract_class_methods(&class.body, &class_name, filepath, line_index, source, symbols);
            }
        }
        Statement::TSInterfaceDeclaration(iface) => {
            symbols.push(Symbol {
                name: iface.id.name.to_string(),
                kind: SymbolKind::Interface,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(iface.span.start),
                end_line: line_index.line_of(iface.span.end),
                doc_comment: extract_leading_comment(source, iface.span.start),
                params: vec![],
            });
        }
        Statement::TSTypeAliasDeclaration(alias) => {
            symbols.push(Symbol {
                name: alias.id.name.to_string(),
                kind: SymbolKind::Type,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(alias.span.start),
                end_line: line_index.line_of(alias.span.end),
                doc_comment: extract_leading_comment(source, alias.span.start),
                params: vec![],
            });
        }
        Statement::TSEnumDeclaration(e) => {
            symbols.push(Symbol {
                name: e.id.name.to_string(),
                kind: SymbolKind::Enum,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(e.span.start),
                end_line: line_index.line_of(e.span.end),
                doc_comment: extract_leading_comment(source, e.span.start),
                params: vec![],
            });
        }
        Statement::ExportDefaultDeclaration(export) => {
            match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                    let name = func
                        .id
                        .as_ref()
                        .map(|id| id.name.to_string())
                        .unwrap_or_else(|| "default".to_string());
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Function,
                        filepath: filepath.to_string(),
                        start_line: line_index.line_of(func.span.start),
                        end_line: line_index.line_of(func.span.end),
                        doc_comment: extract_leading_comment(source, func.span.start),
                        params: extract_params(&func.params),
                    });
                }
                ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    let name = class
                        .id
                        .as_ref()
                        .map(|id| id.name.to_string())
                        .unwrap_or_else(|| "default".to_string());
                    symbols.push(Symbol {
                        name,
                        kind: SymbolKind::Class,
                        filepath: filepath.to_string(),
                        start_line: line_index.line_of(class.span.start),
                        end_line: line_index.line_of(class.span.end),
                        doc_comment: extract_leading_comment(source, class.span.start),
                        params: vec![],
                    });
                }
                _ => {}
            }
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(decl) = &export.declaration {
                extract_from_declaration(decl, filepath, line_index, source, symbols);
            }
        }
        Statement::VariableDeclaration(decl) => {
            extract_from_var_decl(decl, filepath, line_index, source, symbols);
        }
        _ => {}
    }
}

fn extract_from_declaration(
    decl: &Declaration<'_>,
    filepath: &str,
    line_index: &LineIndex,
    source: &str,
    symbols: &mut Vec<Symbol>,
) {
    match decl {
        Declaration::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                symbols.push(Symbol {
                    name: id.name.to_string(),
                    kind: SymbolKind::Function,
                    filepath: filepath.to_string(),
                    start_line: line_index.line_of(func.span.start),
                    end_line: line_index.line_of(func.span.end),
                    doc_comment: extract_leading_comment(source, func.span.start),
                    params: extract_params(&func.params),
                });
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                let class_name = id.name.to_string();
                symbols.push(Symbol {
                    name: class_name.clone(),
                    kind: SymbolKind::Class,
                    filepath: filepath.to_string(),
                    start_line: line_index.line_of(class.span.start),
                    end_line: line_index.line_of(class.span.end),
                    doc_comment: extract_leading_comment(source, class.span.start),
                    params: vec![],
                });
                extract_class_methods(&class.body, &class_name, filepath, line_index, source, symbols);
            }
        }
        Declaration::TSInterfaceDeclaration(iface) => {
            symbols.push(Symbol {
                name: iface.id.name.to_string(),
                kind: SymbolKind::Interface,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(iface.span.start),
                end_line: line_index.line_of(iface.span.end),
                doc_comment: extract_leading_comment(source, iface.span.start),
                params: vec![],
            });
        }
        Declaration::TSTypeAliasDeclaration(alias) => {
            symbols.push(Symbol {
                name: alias.id.name.to_string(),
                kind: SymbolKind::Type,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(alias.span.start),
                end_line: line_index.line_of(alias.span.end),
                doc_comment: extract_leading_comment(source, alias.span.start),
                params: vec![],
            });
        }
        Declaration::TSEnumDeclaration(e) => {
            symbols.push(Symbol {
                name: e.id.name.to_string(),
                kind: SymbolKind::Enum,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(e.span.start),
                end_line: line_index.line_of(e.span.end),
                doc_comment: extract_leading_comment(source, e.span.start),
                params: vec![],
            });
        }
        Declaration::VariableDeclaration(decl) => {
            extract_from_var_decl(decl, filepath, line_index, source, symbols);
        }
        _ => {}
    }
}

fn extract_from_var_decl(
    decl: &VariableDeclaration<'_>,
    filepath: &str,
    line_index: &LineIndex,
    source: &str,
    symbols: &mut Vec<Symbol>,
) {
    for declarator in &decl.declarations {
        // Only extract arrow functions and significant const assignments
        let is_function = declarator.init.as_ref().is_some_and(|init| {
            matches!(
                init,
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
        });

        if !is_function {
            continue;
        }

        if let BindingPatternKind::BindingIdentifier(id) = &declarator.id.kind {
            let params = match declarator.init.as_ref() {
                Some(Expression::ArrowFunctionExpression(arrow)) => extract_params(&arrow.params),
                Some(Expression::FunctionExpression(func)) => extract_params(&func.params),
                _ => vec![],
            };
            symbols.push(Symbol {
                name: id.name.to_string(),
                kind: SymbolKind::Function,
                filepath: filepath.to_string(),
                start_line: line_index.line_of(decl.span.start),
                end_line: line_index.line_of(decl.span.end),
                doc_comment: extract_leading_comment(source, decl.span.start),
                params,
            });
        }
    }
}

fn extract_class_methods(
    body: &ClassBody<'_>,
    _class_name: &str,
    filepath: &str,
    line_index: &LineIndex,
    source: &str,
    symbols: &mut Vec<Symbol>,
) {
    for element in &body.body {
        if let ClassElement::MethodDefinition(method) = element {
            if let Some(name) = method.key.static_name() {
                let params = method
                    .value
                    .params
                    .items
                    .iter()
                    .filter_map(|p| binding_pattern_name(&p.pattern))
                    .collect();
                symbols.push(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::Method,
                    filepath: filepath.to_string(),
                    start_line: line_index.line_of(method.span.start),
                    end_line: line_index.line_of(method.span.end),
                    doc_comment: extract_leading_comment(source, method.span.start),
                    params,
                });
            }
        }
    }
}

fn extract_params(params: &FormalParameters<'_>) -> Vec<String> {
    params
        .items
        .iter()
        .filter_map(|p| binding_pattern_name(&p.pattern))
        .collect()
}

fn binding_pattern_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match &pattern.kind {
        BindingPatternKind::BindingIdentifier(id) => Some(id.name.to_string()),
        _ => None,
    }
}

/// Extract a leading JSDoc/block comment before a given byte offset.
fn extract_leading_comment(source: &str, offset: u32) -> Option<String> {
    let before = &source[..offset as usize];
    let trimmed = before.trim_end();

    if trimmed.ends_with("*/") {
        let start = trimmed.rfind("/*")?;
        let comment = &trimmed[start..];
        // Strip comment markers
        let cleaned: String = comment
            .lines()
            .map(|line| {
                line.trim()
                    .trim_start_matches("/**")
                    .trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim_start_matches('*')
                    .trim()
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    } else {
        // Check for // line comments
        let mut comment_lines = Vec::new();
        for line in before.lines().rev() {
            let trimmed_line = line.trim();
            if trimmed_line.starts_with("//") {
                comment_lines.push(trimmed_line.trim_start_matches("//").trim().to_string());
            } else if trimmed_line.is_empty() {
                continue;
            } else {
                break;
            }
        }
        if comment_lines.is_empty() {
            return None;
        }
        comment_lines.reverse();
        Some(comment_lines.join(" "))
    }
}

/// Maps byte offsets to 1-based line numbers.
struct LineIndex {
    line_starts: Vec<u32>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(i as u32 + 1);
            }
        }
        Self { line_starts }
    }

    fn line_of(&self, offset: u32) -> u32 {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line as u32 + 1,
            Err(line) => line as u32,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_function_declaration() {
        let source = "function greet(name: string): string { return name; }";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].params, vec!["name"]);
    }

    #[test]
    fn test_arrow_function() {
        let source = "const greet = (name: string) => name;";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].kind, SymbolKind::Function);
    }

    #[test]
    fn test_class_with_methods() {
        let source = r#"
class UserService {
    constructor(private db: Database) {}
    findById(id: string): User { return this.db.find(id); }
}
"#;
        let symbols = extract_ts_symbols(source, "test.ts");
        assert!(symbols.iter().any(|s| s.name == "UserService" && s.kind == SymbolKind::Class));
        assert!(symbols.iter().any(|s| s.name == "constructor" && s.kind == SymbolKind::Method));
        assert!(symbols.iter().any(|s| s.name == "findById" && s.kind == SymbolKind::Method));
    }

    #[test]
    fn test_interface() {
        let source = "interface User { id: string; name: string; }";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "User");
        assert_eq!(symbols[0].kind, SymbolKind::Interface);
    }

    #[test]
    fn test_type_alias() {
        let source = "type Result<T> = { ok: true; value: T } | { ok: false; error: Error };";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Result");
        assert_eq!(symbols[0].kind, SymbolKind::Type);
    }

    #[test]
    fn test_export_named() {
        let source = "export function validate(token: string): boolean { return true; }";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "validate");
    }

    #[test]
    fn test_enum() {
        let source = "enum Color { Red, Green, Blue }";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Color");
        assert_eq!(symbols[0].kind, SymbolKind::Enum);
    }

    #[test]
    fn test_jsdoc_comment() {
        let source = r#"
/** Validates a JWT token and returns the payload. */
function validateToken(token: string): Payload { }
"#;
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].doc_comment.as_ref().unwrap().contains("Validates a JWT"));
    }

    #[test]
    fn test_line_numbers() {
        let source = "line1\nline2\nfunction foo() {}\nline4\n";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols[0].start_line, 3);
        assert_eq!(symbols[0].end_line, 3);
    }
}
