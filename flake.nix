{
  description = "The Tokio console: a debugger for async Rust.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/release-21.11";
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        tokio-console = import ./default.nix { inherit pkgs; };
        devShell = import ./shell.nix { inherit pkgs; };
      in
      {
        inherit devShell;
        packages = { inherit tokio-console; };
        defaultPackage = tokio-console;
      });
}
