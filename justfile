default:
    @just --list

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo nextest run --workspace

semgrep:
    semgrep scan --config p/rust --config p/typescript --metrics=off --error --exclude wasm/pkg .

semgrep-sarif:
    semgrep scan --config p/rust --config p/typescript --metrics=off --error --exclude wasm/pkg --sarif-output semgrep.sarif .

snapshot:
    cargo insta test --workspace

coverage:
    cargo llvm-cov nextest --workspace --lcov --output-path lcov.info

coverage-ratchet:
    cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
    scripts/check-coverage-ratchet.sh lcov.info .metrics/coverage-baseline

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

ci: fmt lint test doc

bench:
    cargo bench --workspace

deny:
    cargo deny check

# Module boundary health checks
modules crate="repoask-core":
    cargo modules structure --package {{crate}} --lib

modules-deps crate="repoask-core":
    cargo modules dependencies --package {{crate}} --lib

shear:
    cargo shear

udeps:
    RUSTC_BOOTSTRAP=1 cargo udeps --workspace --all-targets --all-features

# Code similarity detection (requires: cargo install similarity-rs)
# Target: threshold 0.80 once existing duplicates are resolved
similar:
    similarity-rs crates/ --skip-test --threshold 0.90 --exclude benches --print
similar-types:
    similarity-rs crates/ --skip-test --experimental-types --no-functions --threshold 0.95 --print
similar-check:
    similarity-rs crates/ --skip-test --threshold 0.96 --exclude benches --fail-on-duplicates
    similarity-rs crates/ --skip-test --experimental-types --no-functions --threshold 0.95 --fail-on-duplicates

# Build WASM package with size-optimized settings.
# opt-level=z applies to the entire dependency tree via env var.
wasm-build:
    CARGO_PROFILE_RELEASE_OPT_LEVEL=z wasm-pack build wasm --target web

clean:
    cargo clean
    rm -rf mutants.out/ lcov.info
