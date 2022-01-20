{ lib, protobuf, rustPlatform, nix-gitignore }:
let
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

  src = gitignoreSource extraIgnores ../.;

  cargoTOML = lib.importTOML "${src}/tokio-console/Cargo.toml";
in rustPlatform.buildRustPackage rec {
  pname = cargoTOML.package.name;
  version = cargoTOML.package.version;

  nativeBuildInputs = [ protobuf ];

  inherit src;

  cargoLock = { lockFile = "${src}/Cargo.lock"; };

  meta = {
    inherit (cargoTOML.package) description homepage license;
    maintainers = cargoTOML.package.authors;
  };
}
