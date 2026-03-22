default:
    @just --list

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo nextest run --workspace

snapshot:
    cargo insta test --workspace

coverage:
    cargo llvm-cov nextest --workspace --lcov --output-path lcov.info

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

ci: fmt lint test doc

bench:
    cargo bench --workspace

deny:
    cargo deny check

clean:
    cargo clean
    rm -rf mutants.out/ lcov.info
