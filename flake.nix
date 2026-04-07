{
  description = "repoask – code understanding tool for any repository";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;

        # Common source filtering – keep Cargo files, Rust sources, and
        # tree-sitter grammar C/C++ sources that get compiled at build time.
        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter =
            path: type:
            (craneLib.filterCargoSources path type) || (builtins.match ".*\\.(c|cc|cpp|h|hpp)$" path != null);
        };

        # Native deps needed to compile tree-sitter C grammars
        nativeDeps = with pkgs; [ pkg-config ];
        buildDeps =
          with pkgs;
          [ ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.libiconv
          ];

        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = nativeDeps;
          buildInputs = buildDeps;
          # Exclude wasm crate – it needs wasm32 target and bloats native builds
          cargoExtraArgs = "--workspace --exclude repoask-wasm";
        };

        # Build workspace deps first (cache layer) – wasm excluded
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Helper to build a single package
        buildPkg =
          name: extra:
          craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "--package ${name}";
              doCheck = false; # tests run in CI
            }
            // extra
          );

        # CLI binary
        repoask = buildPkg "repoask" { };

        # Library crates (for use as Nix dependencies)
        repoask-core = buildPkg "repoask-core" { };
        repoask-parser = buildPkg "repoask-parser" { };
        repoask-treesitter = buildPkg "repoask-treesitter" { };
        repoask-repo = buildPkg "repoask-repo" { };
      in
      {
        packages = {
          default = repoask;
          inherit
            repoask
            repoask-core
            repoask-parser
            repoask-treesitter
            repoask-repo
            ;
        };

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
            cargo-nextest # test runner
            cargo-deny # dependency audit
            cargo-insta # snapshot testing
            cargo-llvm-cov # coverage reporting
            cargo-shear # static unused/misplaced dependency detection
            cargo-udeps # unused dependency linting
            cargo-modules # module structure visualization
            gitleaks # secret scanning
            just # task runner
            prek # git hooks
            semgrep # SAST scanning
            typos # spell checker

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
      }
    );
}
