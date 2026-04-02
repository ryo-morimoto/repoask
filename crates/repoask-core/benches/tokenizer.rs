//! Tokenizer benchmarks for `repoask-core`.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "benchmark harness")]

fn main() {
    divan::main();
}

const IDENTIFIERS: &[&str] = &[
    "validateJWTToken",
    "XMLHttpRequest",
    "parse_json_response",
    "my-component-name",
    "src/auth/jwt.ts",
];

#[divan::bench(args = IDENTIFIERS)]
fn split_identifier(input: &str) -> Vec<String> {
    repoask_core::tokenizer::split_identifier(input)
}

#[divan::bench(args = [
    "middleware authentication",
    "This section explains how to authenticate with JWT tokens using the client library",
])]
fn tokenize_text(input: &str) -> Vec<String> {
    repoask_core::tokenizer::tokenize_text(input)
}

#[divan::bench(args = IDENTIFIERS)]
fn tokenize_identifier(input: &str) -> Vec<String> {
    repoask_core::tokenizer::tokenize_identifier(input)
}
