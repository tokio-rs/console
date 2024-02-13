{
  description = "The Tokio console: a debugger for async Rust.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs { inherit system overlays; };

          ####################################################################
          #### tokio-console package                                      ####
          ####################################################################
          tokio-console = with pkgs; let
            inherit (nix-gitignore) gitignoreFilterPure withGitignoreFile;
            # Workaround for the builtins.filterSource issue mentioned in
            # https://nixos.org/manual/nix/unstable/expressions/builtins.html
            # Since this might be built from a flake, the source path may be a store path,
            # so we need to provide our own version of gitignoreSource that avoids
            # builtins.filterSource in favor of builtins.path.
            gitignoreSource = patterns: path:
              builtins.path {
                filter =
                  gitignoreFilterPure (_: _: true) (withGitignoreFile patterns path) path;
                path = path;
                name = "src";
              };

            # Ignore some extra things that don't factor into the main build to help with
            # caching.
            extraIgnores = ''
              /.envrc
              /*.nix
              /flake.*
              /netlify.toml
              /.github
              /assets
              /*.md
              /.gitignore
              /LICENSE
            '';

            src = gitignoreSource extraIgnores ./.;

            cargoTOML = lib.importTOML "${src}/tokio-console/Cargo.toml";
            rustToolchain = rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
            rust = makeRustPlatform {
              cargo = rustToolchain;
              rustc = rustToolchain;
            };
          in
          rust.buildRustPackage
            {
              pname = cargoTOML.package.name;
              version = cargoTOML.package.version;

              nativeBuildInputs = [ protobuf ];

              inherit src;

              cargoLock = { lockFile = "${src}/Cargo.lock"; };

              meta = {
                inherit (cargoTOML.package) description homepage license;
                maintainers = cargoTOML.package.authors;
              };
            };

          ####################################################################
          #### dev shell                                                  ####
          ####################################################################
          devShell = with pkgs;
            mkShell {
              name = "tokio-console-env";
              buildInputs = tokio-console.buildInputs ++ lib.optional stdenv.isDarwin libiconv;
              nativeBuildInputs = tokio-console.nativeBuildInputs;
              RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
              CARGO_TERM_COLOR = "always";
              RUST_BACKTRACE = "full";
            };
        in
        {
          apps = {
            tokio-console = {
              type = "app";
              program = "${tokio-console}/bin/tokio-console";
              description = "The Tokio console: a debugger for async Rust.";
            };
            default = self.apps.${system}.tokio-console;
          };
          devShells.default = devShell;
          packages = {
            inherit tokio-console;
            default = self.packages.${system}.tokio-console;
          };
        });
}
