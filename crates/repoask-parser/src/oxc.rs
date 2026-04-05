#![allow(
    clippy::wildcard_imports,
    reason = "oxc AST traversal uses many generated node types"
)]

use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::Comment;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};
use repoask_core::types::{
    CommentInfo, CommentSource, ExportInfo, ExportKind, IndexDocument, Publicness, Reexport,
    Symbol, SymbolKind,
};

/// Extract symbols from TypeScript/JavaScript source code using `oxc_parser`.
#[must_use]
pub fn extract_ts_symbols(source: &str, filepath: &str) -> Vec<Symbol> {
    extract_ts_context(source, filepath).symbols
}

/// Extract parser documents from TypeScript/JavaScript source code using `oxc_parser`.
#[must_use]
pub fn extract_ts_documents(source: &str, filepath: &str) -> Vec<IndexDocument> {
    extract_ts_context(source, filepath).into_documents()
}

fn extract_ts_context<'a>(source: &'a str, filepath: &'a str) -> ExtractCtx<'a> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filepath).unwrap_or_default();

    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        return ExtractCtx::new(filepath, source, HashMap::new());
    }

    let comments = build_comment_map(source, ret.program.comments.as_slice());
    let mut ctx = ExtractCtx::new(filepath, source, comments);

    for stmt in &ret.program.body {
        extract_from_statement(stmt, &mut ctx);
    }

    ctx
}

/// Build a map from token start offset → cleaned doc comment text.
///
/// Uses oxc's `Comment.attached_to` field which gives the start offset of
/// the token the comment is attached to. This replaces the previous
/// `O(file_size)` reverse-scan per symbol with O(1) `HashMap` lookup.
fn build_comment_map(source: &str, comments: &[Comment]) -> HashMap<u32, CommentInfo> {
    let mut map: HashMap<u32, Vec<&Comment>> = HashMap::new();
    for comment in comments {
        map.entry(comment.attached_to).or_default().push(comment);
    }

    let mut result = HashMap::new();
    for (attached_to, group) in map {
        if let Some(comment) = normalize_comment_group(source, &group) {
            result.insert(attached_to, comment);
        }
    }
    result
}

/// Clean and join a group of comments attached to the same token.
fn normalize_comment_group(source: &str, comments: &[&Comment]) -> Option<CommentInfo> {
    let mut parts = Vec::new();
    for comment in comments {
        let text = &source[comment.span.start as usize..comment.span.end as usize];
        let cleaned = clean_comment_text(text);
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }

    let source = if comments.iter().any(|comment| {
        source[comment.span.start as usize..comment.span.end as usize]
            .trim_start()
            .starts_with("/**")
    }) {
        CommentSource::JsDoc
    } else {
        CommentSource::PlainComment
    };

    CommentInfo::from_normalized_text(&parts.join(" "), source)
}

/// Clean a single comment's raw text (strip delimiters and asterisks).
fn clean_comment_text(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            line.trim()
                .trim_start_matches("/**")
                .trim_start_matches("/*")
                .trim_start_matches("//")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shared context threaded through all extraction functions.
struct ExtractCtx<'a> {
    filepath: &'a str,
    line_index: LineIndex,
    /// Pre-computed doc comments keyed by token start offset.
    comments: HashMap<u32, CommentInfo>,
    symbols: Vec<Symbol>,
    reexports: Vec<Reexport>,
}

impl<'a> ExtractCtx<'a> {
    fn new(filepath: &'a str, source: &'a str, comments: HashMap<u32, CommentInfo>) -> Self {
        Self {
            filepath,
            line_index: LineIndex::new(source),
            comments,
            symbols: Vec::new(),
            reexports: Vec::new(),
        }
    }

    fn into_documents(self) -> Vec<IndexDocument> {
        self.symbols
            .into_iter()
            .map(IndexDocument::Code)
            .chain(self.reexports.into_iter().map(IndexDocument::Reexport))
            .collect()
    }

    /// Push a symbol with common field wiring.
    fn push(
        &mut self,
        name: String,
        kind: SymbolKind,
        span: Span,
        params: Vec<String>,
        export: ExportInfo,
    ) {
        let signature_preview = Some(build_signature_preview(kind, &name, &params));
        self.symbols.push(Symbol {
            name,
            kind,
            filepath: self.filepath.to_owned(),
            start_line: self.line_index.line_of(span.start),
            end_line: self.line_index.line_of(span.end),
            params,
            signature_preview,
            comment: self.comments.get(&span.start).cloned(),
            export,
        });
    }

    fn push_reexport(
        &mut self,
        span: Span,
        local_name: String,
        exported_name: String,
        source_specifier: Option<String>,
        is_type_only: bool,
    ) {
        let (start_line, end_line) = self.span_lines(span);
        self.reexports.push(Reexport {
            filepath: self.filepath.to_owned(),
            start_line,
            end_line,
            local_name,
            exported_name,
            source_specifier,
            is_type_only,
        });
    }

    fn span_lines(&self, span: Span) -> (u32, u32) {
        (
            self.line_index.line_of(span.start),
            self.line_index.line_of(span.end),
        )
    }
}

fn extract_from_statement(stmt: &Statement<'_>, ctx: &mut ExtractCtx<'_>) {
    // Delegate declarations shared between Statement and Declaration enums
    if let Some(decl) = stmt.as_declaration() {
        extract_from_declaration(decl, ctx, &ExportInfo::private());
        return;
    }

    match stmt {
        Statement::ExportDefaultDeclaration(export) => {
            extract_from_export_default(&export.declaration, ctx);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(decl) = &export.declaration {
                extract_from_declaration(decl, ctx, &ExportInfo::public_named());
            } else {
                extract_reexports(export, ctx);
            }
        }
        Statement::ExportAllDeclaration(export) => {
            extract_export_all(export, ctx);
        }
        _ => {}
    }
}

fn extract_reexports(export: &ExportNamedDeclaration<'_>, ctx: &mut ExtractCtx<'_>) {
    let source_specifier = export
        .source
        .as_ref()
        .map(|source| source.value.to_string());
    let declaration_is_type_only = export.export_kind == ImportOrExportKind::Type;

    for specifier in &export.specifiers {
        ctx.push_reexport(
            specifier.span,
            specifier.local.name().to_string(),
            specifier.exported.name().to_string(),
            source_specifier.clone(),
            declaration_is_type_only || specifier.export_kind == ImportOrExportKind::Type,
        );
    }
}

fn extract_export_all(export: &ExportAllDeclaration<'_>, ctx: &mut ExtractCtx<'_>) {
    ctx.push_reexport(
        export.span,
        "*".to_owned(),
        export
            .exported
            .as_ref()
            .map_or_else(|| "*".to_owned(), |name| name.name().to_string()),
        Some(export.source.value.to_string()),
        export.export_kind == ImportOrExportKind::Type,
    );
}

fn extract_from_export_default(
    export: &ExportDefaultDeclarationKind<'_>,
    ctx: &mut ExtractCtx<'_>,
) {
    match export {
        ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            let name = func
                .id
                .as_ref()
                .map_or_else(|| "default".to_owned(), |id| id.name.to_string());
            ctx.push(
                name,
                SymbolKind::Function,
                func.span,
                extract_params(&func.params),
                ExportInfo::public_default(),
            );
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            let name = class
                .id
                .as_ref()
                .map_or_else(|| "default".to_owned(), |id| id.name.to_string());
            ctx.push(
                name,
                SymbolKind::Class,
                class.span,
                vec![],
                ExportInfo::public_default(),
            );
        }
        _ => {}
    }
}

fn extract_from_declaration(decl: &Declaration<'_>, ctx: &mut ExtractCtx<'_>, export: &ExportInfo) {
    match decl {
        Declaration::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                ctx.push(
                    id.name.to_string(),
                    SymbolKind::Function,
                    func.span,
                    extract_params(&func.params),
                    export.clone(),
                );
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                let class_name = id.name.to_string();
                ctx.push(
                    class_name.clone(),
                    SymbolKind::Class,
                    class.span,
                    vec![],
                    export.clone(),
                );
                extract_class_methods(&class.body, &class_name, ctx);
            }
        }
        Declaration::TSInterfaceDeclaration(iface) => {
            ctx.push(
                iface.id.name.to_string(),
                SymbolKind::Interface,
                iface.span,
                vec![],
                export.clone(),
            );
        }
        Declaration::TSTypeAliasDeclaration(alias) => {
            ctx.push(
                alias.id.name.to_string(),
                SymbolKind::Type,
                alias.span,
                vec![],
                export.clone(),
            );
        }
        Declaration::TSEnumDeclaration(e) => {
            ctx.push(
                e.id.name.to_string(),
                SymbolKind::Enum,
                e.span,
                vec![],
                export.clone(),
            );
        }
        Declaration::VariableDeclaration(decl) => {
            extract_from_var_decl(decl, ctx, export);
        }
        _ => {}
    }
}

fn extract_from_var_decl(
    decl: &VariableDeclaration<'_>,
    ctx: &mut ExtractCtx<'_>,
    export: &ExportInfo,
) {
    for declarator in &decl.declarations {
        let is_function = declarator.init.as_ref().is_some_and(|init| {
            matches!(
                init,
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
        });

        if !is_function {
            continue;
        }

        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
            let params = match declarator.init.as_ref() {
                Some(Expression::ArrowFunctionExpression(arrow)) => extract_params(&arrow.params),
                Some(Expression::FunctionExpression(func)) => extract_params(&func.params),
                _ => vec![],
            };
            ctx.push(
                id.name.to_string(),
                SymbolKind::Function,
                decl.span,
                params,
                export.clone(),
            );
        }
    }
}

fn extract_class_methods(body: &ClassBody<'_>, class_name: &str, ctx: &mut ExtractCtx<'_>) {
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
                ctx.push(
                    name.to_string(),
                    SymbolKind::Method,
                    method.span,
                    params,
                    ExportInfo {
                        publicness: Publicness::Private,
                        export_kind: ExportKind::ModuleMember,
                        container: Some(class_name.to_owned()),
                    },
                );
            }
        }
    }
}

fn build_signature_preview(kind: SymbolKind, name: &str, params: &[String]) -> String {
    match kind {
        SymbolKind::Function | SymbolKind::Method => format!("{name}({})", params.join(", ")),
        _ => name.to_owned(),
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
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id.name.to_string()),
        _ => None,
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
                line_starts.push(line_number_1based(i));
            }
        }
        Self { line_starts }
    }

    fn line_of(&self, offset: u32) -> u32 {
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line_number_1based(line),
            Err(line) => saturating_u32(line),
        }
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn line_number_1based(value: usize) -> u32 {
    saturating_u32(value).saturating_add(1)
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
        let source = r"
class UserService {
    constructor(private db: Database) {}
    findById(id: string): User { return this.db.find(id); }
}
";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "UserService" && s.kind == SymbolKind::Class)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "constructor" && s.kind == SymbolKind::Method)
        );
        assert!(
            symbols
                .iter()
                .any(|s| s.name == "findById" && s.kind == SymbolKind::Method)
        );
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
        assert_eq!(symbols[0].export.publicness, Publicness::Public);
        assert_eq!(symbols[0].export.export_kind, ExportKind::Named);
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
        let source = r"
/** Validates a JWT token and returns the payload. */
function validateToken(token: string): Payload { }
";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols.len(), 1);
        assert!(
            symbols[0]
                .comment
                .as_ref()
                .and_then(|comment| comment.summary_line.as_deref())
                .unwrap()
                .contains("Validates a JWT")
        );
    }

    #[test]
    fn test_non_exported_function_is_private() {
        let source = "function validate(token: string): boolean { return true; }";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols[0].export.publicness, Publicness::Private);
        assert_eq!(symbols[0].export.export_kind, ExportKind::None);
    }

    #[test]
    fn test_export_specifier_from_other_module_creates_reexport_document() {
        let docs = extract_ts_documents(
            "export { validateToken as createSession } from './auth';",
            "src/index.ts",
        );

        assert!(matches!(
            &docs[0],
            IndexDocument::Reexport(reexport)
                if reexport.local_name == "validateToken"
                    && reexport.exported_name == "createSession"
                    && reexport.source_specifier.as_deref() == Some("./auth")
        ));
    }

    #[test]
    fn test_export_all_creates_wildcard_reexport_document() {
        let docs = extract_ts_documents("export * from './auth';", "src/index.ts");

        assert!(matches!(
            &docs[0],
            IndexDocument::Reexport(reexport)
                if reexport.local_name == "*"
                    && reexport.exported_name == "*"
                    && reexport.source_specifier.as_deref() == Some("./auth")
        ));
    }

    #[test]
    fn test_export_namespace_creates_namespace_reexport_document() {
        let docs = extract_ts_documents("export * as authApi from './auth';", "src/index.ts");

        assert!(matches!(
            &docs[0],
            IndexDocument::Reexport(reexport)
                if reexport.local_name == "*"
                    && reexport.exported_name == "authApi"
                    && reexport.source_specifier.as_deref() == Some("./auth")
        ));
    }

    #[test]
    fn test_line_numbers() {
        let source = "line1\nline2\nfunction foo() {}\nline4\n";
        let symbols = extract_ts_symbols(source, "test.ts");
        assert_eq!(symbols[0].start_line, 3);
        assert_eq!(symbols[0].end_line, 3);
    }

    // -----------------------------------------------------------------------
    // Snapshot tests (insta)
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_mixed_typescript() {
        let source = r"
/** User authentication service */
export class AuthService {
    constructor(private db: Database) {}
    async validateToken(token: string): Promise<User> {
        return this.db.findByToken(token);
    }
}

export interface AuthConfig {
    secret: string;
    expiry: number;
}

export type AuthResult = { ok: true; user: User } | { ok: false; error: string };

export const createAuth = (config: AuthConfig) => new AuthService(config);

enum Role { Admin, User, Guest }
";
        let symbols = extract_ts_symbols(source, "src/auth.ts");
        insta::assert_json_snapshot!(symbols);
    }
}
