//! Module-resolution helpers for investigation surfaces.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use repoask_core::investigation::{ModuleResolutionConfig, PathAliasRule, ScopedModuleResolution};
use serde_json::Value;

/// Read module-resolution hints from repository config files.
#[must_use]
pub fn read_module_resolution(repo_root: &Path) -> ModuleResolutionConfig {
    let repo_root = normalize_path(repo_root);
    let mut config_paths = Vec::new();
    discover_config_paths(&repo_root, &mut config_paths);
    config_paths.sort_by_key(|left| config_sort_key(left));

    let mut config = ModuleResolutionConfig::default();
    let mut seen_scopes = HashSet::new();

    for config_path in config_paths {
        let Some(scoped) = read_scoped_config(&repo_root, &config_path) else {
            continue;
        };
        if !seen_scopes.insert(scoped.scope_dir.clone()) {
            continue;
        }

        if scoped.scope_dir.is_empty() {
            config.tsconfig_base_url = scoped.tsconfig_base_url;
            config.tsconfig_paths = scoped.tsconfig_paths;
        } else {
            config.scoped_configs.push(scoped);
        }
    }

    config
}

#[derive(Debug, Default)]
struct ResolvedModuleConfig {
    tsconfig_base_url: Option<String>,
    tsconfig_paths: Vec<PathAliasRule>,
}

fn read_scoped_config(repo_root: &Path, config_path: &Path) -> Option<ScopedModuleResolution> {
    let mut visited = HashSet::new();
    let resolved = read_config_file_recursive(repo_root, config_path, &mut visited)?;
    let scope_dir = normalize_scope_dir(config_path.parent().unwrap_or(repo_root), repo_root)?;

    Some(ScopedModuleResolution {
        scope_dir,
        tsconfig_base_url: resolved.tsconfig_base_url,
        tsconfig_paths: resolved.tsconfig_paths,
    })
}

fn read_config_file_recursive(
    repo_root: &Path,
    config_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Option<ResolvedModuleConfig> {
    let config_path = normalize_path(config_path);
    if !visited.insert(config_path.clone()) {
        return Some(ResolvedModuleConfig::default());
    }

    let source = std::fs::read_to_string(&config_path).ok()?;
    let json = parse_jsonc_value(&source)?;
    let config_dir = config_path.parent().unwrap_or(repo_root);

    let parent = json
        .get("extends")
        .and_then(Value::as_str)
        .and_then(|extends| resolve_extends_path(repo_root, config_dir, extends))
        .and_then(|path| read_config_file_recursive(repo_root, &path, visited))
        .unwrap_or_default();

    let current = extract_compiler_options(repo_root, config_dir, &json);
    Some(merge_configs(parent, current))
}

fn extract_compiler_options(
    repo_root: &Path,
    config_dir: &Path,
    json: &Value,
) -> ResolvedModuleConfig {
    let compiler_options = json.get("compilerOptions").and_then(Value::as_object);

    let tsconfig_base_url = compiler_options
        .and_then(|options| options.get("baseUrl"))
        .and_then(Value::as_str)
        .and_then(|base_url| normalize_repo_relative(config_dir, repo_root, base_url));

    let tsconfig_paths = compiler_options
        .and_then(|options| options.get("paths"))
        .and_then(Value::as_object)
        .map(|paths| {
            paths
                .iter()
                .filter_map(|(pattern, value)| {
                    let targets = value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .filter_map(|target| normalize_repo_relative(config_dir, repo_root, target))
                        .collect::<Vec<_>>();
                    (!targets.is_empty()).then(|| PathAliasRule {
                        pattern: pattern.clone(),
                        targets,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ResolvedModuleConfig {
        tsconfig_base_url,
        tsconfig_paths,
    }
}

fn merge_configs(
    parent: ResolvedModuleConfig,
    current: ResolvedModuleConfig,
) -> ResolvedModuleConfig {
    let mut merged_paths = current.tsconfig_paths;
    for rule in parent.tsconfig_paths {
        if !merged_paths
            .iter()
            .any(|candidate| candidate.pattern == rule.pattern)
        {
            merged_paths.push(rule);
        }
    }

    ResolvedModuleConfig {
        tsconfig_base_url: current.tsconfig_base_url.or(parent.tsconfig_base_url),
        tsconfig_paths: merged_paths,
    }
}

fn resolve_extends_path(repo_root: &Path, config_dir: &Path, extends: &str) -> Option<PathBuf> {
    if extends.starts_with('.') || extends.starts_with('/') {
        let candidate = normalize_path(config_dir.join(extends));
        if candidate.exists() {
            return Some(candidate);
        }
        let with_json = candidate.with_extension("json");
        return with_json.exists().then_some(with_json);
    }
    let mut current = Some(config_dir);
    while let Some(dir) = current {
        let package_candidate = normalize_path(dir.join("node_modules").join(extends));
        if package_candidate.exists() {
            return Some(package_candidate);
        }
        let with_json = package_candidate.with_extension("json");
        if with_json.exists() {
            return Some(with_json);
        }

        if dir == repo_root {
            break;
        }
        current = dir.parent();
    }

    None
}

fn discover_config_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | "node_modules" | "target" | ".direnv"
            ) {
                continue;
            }
            discover_config_paths(&path, out);
            continue;
        }

        let file_name = entry.file_name();
        if matches!(
            file_name.to_string_lossy().as_ref(),
            "tsconfig.json" | "jsconfig.json"
        ) {
            out.push(path);
        }
    }
}

fn config_sort_key(path: &Path) -> (usize, u8, String) {
    let depth = path.components().count();
    let priority = match path.file_name().and_then(|name| name.to_str()) {
        Some("tsconfig.json") => 0,
        Some("jsconfig.json") => 1,
        _ => 2,
    };
    (depth, priority, path.to_string_lossy().to_string())
}

fn normalize_scope_dir(scope_dir: &Path, repo_root: &Path) -> Option<String> {
    let relative = normalize_path(scope_dir)
        .strip_prefix(repo_root)
        .ok()?
        .to_path_buf();
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_repo_relative(base_dir: &Path, repo_root: &Path, value: &str) -> Option<String> {
    let joined = normalize_path(base_dir.join(value));
    let relative = joined.strip_prefix(repo_root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(std::path::MAIN_SEPARATOR.to_string()),
        }
    }
    normalized
}

fn parse_jsonc_value(source: &str) -> Option<Value> {
    let without_comments = strip_jsonc_comments(source);
    let normalized = strip_trailing_commas(&without_comments);
    serde_json::from_str(&normalized).ok()
}

fn strip_jsonc_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaping = false;

    while let Some(ch) = chars.next() {
        if in_string {
            result.push(ch);
            if escaping {
                escaping = false;
            } else if ch == '\\' {
                escaping = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            result.push(ch);
            continue;
        }

        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    for comment_char in chars.by_ref() {
                        if comment_char == '\n' {
                            result.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut prev = '\0';
                    for comment_char in chars.by_ref() {
                        if comment_char == '\n' {
                            result.push('\n');
                        }
                        if prev == '*' && comment_char == '/' {
                            break;
                        }
                        prev = comment_char;
                    }
                    continue;
                }
                _ => {}
            }
        }

        result.push(ch);
    }

    result
}

fn strip_trailing_commas(source: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let mut result = String::with_capacity(source.len());
    let mut in_string = false;
    let mut escaping = false;

    for (index, ch) in chars.iter().enumerate() {
        if in_string {
            result.push(*ch);
            if escaping {
                escaping = false;
            } else if *ch == '\\' {
                escaping = true;
            } else if *ch == '"' {
                in_string = false;
            }
            continue;
        }

        if *ch == '"' {
            in_string = true;
            result.push(*ch);
            continue;
        }

        if *ch == ',' {
            let next_non_ws = chars[index + 1..]
                .iter()
                .find(|candidate| !candidate.is_whitespace())
                .copied();
            if matches!(next_non_ws, Some('}' | ']')) {
                continue;
            }
        }

        result.push(*ch);
    }

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test assertions")]
mod tests {
    use super::*;

    #[test]
    fn reads_base_url_and_paths_from_tsconfig() {
        let repo_root = tempfile::tempdir().unwrap();
        let tsconfig = repo_root.path().join("tsconfig.json");
        std::fs::write(
            &tsconfig,
            r#"{
                "compilerOptions": {
                    "baseUrl": ".",
                    "paths": {
                        "@/*": ["src/*"],
                        "@lib": ["lib/index.ts"]
                    }
                }
            }"#,
        )
        .unwrap();

        let config = read_module_resolution(repo_root.path());

        assert_eq!(config.tsconfig_base_url.as_deref(), Some(""));
        assert!(
            config
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@/*" && rule.targets == vec!["src/*"])
        );
        assert!(
            config
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@lib" && rule.targets == vec!["lib/index.ts"])
        );
    }

    #[test]
    fn reads_nested_config_with_extends_and_scope_precedence() {
        let repo_root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo_root.path().join("packages/web")).unwrap();

        std::fs::write(
            repo_root.path().join("tsconfig.base.json"),
            r#"{
                "compilerOptions": {
                    "baseUrl": ".",
                    "paths": {
                        "@shared/*": ["shared/*"]
                    }
                }
            }"#,
        )
        .unwrap();
        std::fs::write(
            repo_root.path().join("tsconfig.json"),
            r#"{
                "extends": "./tsconfig.base.json",
                "compilerOptions": {
                    "paths": {
                        "@/*": ["src/*"]
                    }
                }
            }"#,
        )
        .unwrap();
        std::fs::write(
            repo_root.path().join("packages/web/tsconfig.json"),
            r#"{
                "extends": "../../tsconfig.base.json",
                "compilerOptions": {
                    "baseUrl": ".",
                    "paths": {
                        "@/*": ["src/*"]
                    }
                }
            }"#,
        )
        .unwrap();

        let config = read_module_resolution(repo_root.path());

        assert!(
            config
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@/*" && rule.targets == vec!["src/*"])
        );
        let web_scope = config
            .scoped_configs
            .iter()
            .find(|scope| scope.scope_dir == "packages/web")
            .unwrap();
        assert_eq!(web_scope.tsconfig_base_url.as_deref(), Some("packages/web"));
        assert!(
            web_scope
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@/*" && rule.targets == vec!["packages/web/src/*"])
        );
        assert!(
            web_scope
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@shared/*" && rule.targets == vec!["shared/*"])
        );
    }

    #[test]
    fn parses_jsonc_comments_and_trailing_commas() {
        let repo_root = tempfile::tempdir().unwrap();
        std::fs::write(
            repo_root.path().join("tsconfig.json"),
            r#"{
                // comment
                "compilerOptions": {
                    "baseUrl": ".",
                    "paths": {
                        "@/*": ["src/*",],
                    },
                },
            }"#,
        )
        .unwrap();

        let config = read_module_resolution(repo_root.path());

        assert_eq!(config.tsconfig_base_url.as_deref(), Some(""));
        assert!(
            config
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@/*" && rule.targets == vec!["src/*"])
        );
    }

    #[test]
    fn resolves_package_extends_from_ancestor_node_modules() {
        let repo_root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo_root.path().join("node_modules/@acme/config")).unwrap();
        std::fs::create_dir_all(repo_root.path().join("packages/web")).unwrap();

        std::fs::write(
            repo_root.path().join("node_modules/@acme/config/base.json"),
            r#"{
                "compilerOptions": {
                    "paths": {
                        "@shared/*": ["shared/*"]
                    }
                }
            }"#,
        )
        .unwrap();
        std::fs::write(
            repo_root.path().join("packages/web/tsconfig.json"),
            r#"{
                "extends": "@acme/config/base",
                "compilerOptions": {
                    "baseUrl": "."
                }
            }"#,
        )
        .unwrap();

        let config = read_module_resolution(repo_root.path());
        let web_scope = config
            .scoped_configs
            .iter()
            .find(|scope| scope.scope_dir == "packages/web")
            .unwrap();

        assert!(
            web_scope
                .tsconfig_paths
                .iter()
                .any(|rule| rule.pattern == "@shared/*"
                    && rule.targets == vec!["node_modules/@acme/config/shared/*"])
        );
    }
}
