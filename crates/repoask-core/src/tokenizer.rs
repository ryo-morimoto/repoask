use rust_stemmers::{Algorithm, Stemmer};

/// Split a code identifier into lowercase tokens.
///
/// Handles camelCase, PascalCase, snake_case, kebab-case, and
/// consecutive uppercase runs (e.g. "parseJSON" → ["parse", "json"]).
pub fn split_identifier(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = name.chars().collect();

    for i in 0..chars.len() {
        let ch = chars[i];

        if ch == '_' || ch == '-' || ch == '.' || ch == '/' {
            if !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
            continue;
        }

        if ch.is_uppercase() {
            let prev_lower = i > 0 && chars[i - 1].is_lowercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
            let curr_upper_run = i > 0 && chars[i - 1].is_uppercase();

            // Split before: aB, or end of uppercase run ABc
            if prev_lower || (curr_upper_run && next_lower) {
                if !current.is_empty() {
                    tokens.push(current.to_lowercase());
                    current.clear();
                }
            }
        }

        current.push(ch);
    }

    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

/// Tokenize natural language text into lowercase stemmed tokens.
pub fn tokenize_text(text: &str) -> Vec<String> {
    let stemmer = Stemmer::create(Algorithm::English);

    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() > 1 && w.len() < 80)
        .map(|w| {
            let lower = w.to_lowercase();
            stemmer.stem(&lower).to_string()
        })
        .collect()
}

/// Tokenize a code identifier: split then stem each part.
pub fn tokenize_identifier(name: &str) -> Vec<String> {
    let stemmer = Stemmer::create(Algorithm::English);

    split_identifier(name)
        .into_iter()
        .filter(|t| t.len() > 1)
        .map(|t| stemmer.stem(&t).to_string())
        .collect()
}

/// Tokenize a query string (same pipeline as text, for consistent matching).
pub fn tokenize_query(query: &str) -> Vec<String> {
    tokenize_text(query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camel_case() {
        assert_eq!(split_identifier("validateToken"), vec!["validate", "token"]);
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(split_identifier("ValidateToken"), vec!["validate", "token"]);
    }

    #[test]
    fn test_snake_case() {
        assert_eq!(
            split_identifier("validate_jwt_token"),
            vec!["validate", "jwt", "token"]
        );
    }

    #[test]
    fn test_uppercase_run() {
        assert_eq!(
            split_identifier("parseJSONResponse"),
            vec!["parse", "json", "response"]
        );
    }

    #[test]
    fn test_all_uppercase() {
        assert_eq!(split_identifier("HTTP"), vec!["http"]);
    }

    #[test]
    fn test_mixed() {
        assert_eq!(
            split_identifier("XMLHttpRequest"),
            vec!["xml", "http", "request"]
        );
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!(
            split_identifier("my-component-name"),
            vec!["my", "component", "name"]
        );
    }

    #[test]
    fn test_filepath_tokens() {
        assert_eq!(
            split_identifier("src/auth/jwt.ts"),
            vec!["src", "auth", "jwt", "ts"]
        );
    }

    #[test]
    fn test_tokenize_identifier_stems() {
        let tokens = tokenize_identifier("validateJWTToken");
        // "validate" stems to "valid", "jwt" stays, "token" stays
        assert!(tokens.contains(&"valid".to_string()));
        assert!(tokens.contains(&"jwt".to_string()));
        assert!(tokens.contains(&"token".to_string()));
    }

    #[test]
    fn test_tokenize_text() {
        let tokens = tokenize_text("middleware authentication setup");
        assert_eq!(tokens.len(), 3);
        // All should be stemmed
        assert!(tokens.contains(&"middlewar".to_string()) || tokens.contains(&"middleware".to_string()));
    }
}
