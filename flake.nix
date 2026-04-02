{
  description = "repoask development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # === Rust toolchain ===
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer

            # === WASM target ===
            wasm-pack
            wasm-bindgen-cli
            binaryen # wasm-opt

            # === Build dependencies ===
            # tree-sitter grammars compile C code
            gcc
            pkg-config
            libiconv # macOS compat
            lld # WASM linker for cdylib targets
            llvmPackages_21.llvm # llvm-cov / llvm-profdata for coverage

            # === Git (for repoask-repo clone operations) ===
            git

            # === Dev tools (HARNESS Layer 2-4) ===
            cargo-nextest  # test runner
            cargo-deny     # dependency audit
            cargo-insta    # snapshot testing
            cargo-llvm-cov # coverage reporting
            cargo-machete  # unused dependency detection
            cargo-modules  # module structure visualization
            gitleaks       # secret scanning
            just           # task runner
            prek           # git hooks
            typos          # spell checker

            # === Documentation ===
            mdbook # if we add a docs site later
          ];

          # Rust source for IDE support
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          LLVM_COV = "${pkgs.llvmPackages_21.llvm}/bin/llvm-cov";
          LLVM_PROFDATA = "${pkgs.llvmPackages_21.llvm}/bin/llvm-profdata";

          shellHook = ''
            # Install git hooks on shell entry if not already installed
            if [ ! -f .git/hooks/pre-commit ] || ! grep -q prek .git/hooks/pre-commit 2>/dev/null; then
              prek install --hook-type pre-commit 2>/dev/null || true
              prek install --hook-type pre-push 2>/dev/null || true
              prek install --hook-type commit-msg 2>/dev/null || true
            fi
          '';
        };
      });
}
