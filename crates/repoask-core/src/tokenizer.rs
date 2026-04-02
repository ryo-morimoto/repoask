use std::cell::RefCell;

use rust_stemmers::{Algorithm, Stemmer};

thread_local! {
    static STEMMER: RefCell<Stemmer> = RefCell::new(Stemmer::create(Algorithm::English));
}

/// Split a code identifier into lowercase tokens.
///
/// Handles `camelCase`, `PascalCase`, `snake_case`, `kebab-case`, and
/// consecutive uppercase runs (e.g. `parseJSON` -> `["parse", "json"]`).
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
            if (prev_lower || (curr_upper_run && next_lower)) && !current.is_empty() {
                tokens.push(current.to_lowercase());
                current.clear();
            }
        }

        current.push(ch);
    }

    if !current.is_empty() {
        tokens.push(current.to_lowercase());
    }

    tokens
}

/// Stem a sequence of lowercase tokens using the shared stemmer.
fn stem_tokens(tokens: impl Iterator<Item = String>) -> Vec<String> {
    STEMMER.with(|s| {
        let stemmer = s.borrow();
        tokens
            .filter(|t| t.len() > 1)
            .map(|t| stemmer.stem(&t).to_string())
            .collect()
    })
}

/// Tokenize natural language text into lowercase stemmed tokens.
pub fn tokenize_text(text: &str) -> Vec<String> {
    let words = text
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() < 80)
        .map(str::to_lowercase);
    stem_tokens(words)
}

/// Tokenize a code identifier: split then stem each part.
pub fn tokenize_identifier(name: &str) -> Vec<String> {
    stem_tokens(split_identifier(name).into_iter())
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
        assert!(tokens.contains(&"valid".to_owned()));
        assert!(tokens.contains(&"jwt".to_owned()));
        assert!(tokens.contains(&"token".to_owned()));
    }

    #[test]
    fn test_tokenize_text() {
        let tokens = tokenize_text("middleware authentication setup");
        assert_eq!(tokens.len(), 3);
        // Each token should be the Porter-stemmed form of the input word
        let stemmer = Stemmer::create(Algorithm::English);
        for word in ["middleware", "authentication", "setup"] {
            let expected = stemmer.stem(&word.to_lowercase()).to_string();
            assert!(
                tokens.contains(&expected),
                "expected stemmed form {expected:?} of {word:?} in {tokens:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property-based tests (proptest)
    // -----------------------------------------------------------------------

    mod property {
        use super::*;
        use proptest::prelude::*;

        /// Generate random camelCase / `snake_case` identifiers.
        fn identifier_strategy() -> impl Strategy<Value = String> {
            prop::collection::vec("[a-z]{2,8}", 1..5).prop_map(|parts| {
                let mut result = parts[0].clone();
                for part in &parts[1..] {
                    append_identifier_part(&mut result, part);
                }
                result
            })
        }

        fn append_identifier_part(result: &mut String, part: &str) {
            // Randomly mix camelCase and snake_case
            if result.len() % 2 != 0 {
                result.push('_');
                result.push_str(part);
                return;
            }

            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return;
            };
            result.push(first.to_ascii_uppercase());
            result.extend(chars);
        }

        proptest! {
            /// split_identifier never produces empty tokens.
            #[test]
            fn split_never_empty_tokens(name in identifier_strategy()) {
                let tokens = split_identifier(&name);
                for token in &tokens {
                    prop_assert!(!token.is_empty(), "empty token from: {name}");
                }
            }

            /// split_identifier tokens are always lowercase.
            #[test]
            fn split_always_lowercase(name in identifier_strategy()) {
                let tokens = split_identifier(&name);
                for token in &tokens {
                    prop_assert_eq!(token, &token.to_lowercase(), "non-lowercase token from: {}", name);
                }
            }

            /// All characters from the original identifier appear in some token.
            #[test]
            fn split_preserves_all_alpha_chars(name in identifier_strategy()) {
                let tokens = split_identifier(&name);
                let joined: String = tokens.concat();
                for ch in name.chars() {
                    if ch.is_alphanumeric() {
                        prop_assert!(
                            joined.contains(ch.to_ascii_lowercase()),
                            "lost char '{ch}' from: {name}"
                        );
                    }
                }
            }

            /// tokenize_query on ASCII input never panics and returns only non-empty tokens.
            #[test]
            fn tokenize_query_no_panic(query in "[a-zA-Z0-9_ ]{0,200}") {
                let tokens = tokenize_query(&query);
                for token in &tokens {
                    prop_assert!(!token.is_empty());
                }
            }
        }
    }
}
