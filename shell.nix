scope@{ pkgs ? import <nixpkgs> { } }:
with pkgs;
let
  tokio-console = import ./default.nix { inherit pkgs; };

  env = buildEnv {
    name = "console-env";
    paths = [ ]
      ++ lib.optional stdenv.isDarwin libiconv
      ++ tokio-console.buildInputs
      ++ tokio-console.nativeBuildInputs;
  };
in
mkShell {
  buildInputs = [ env ];

  CARGO_TERM_COLOR = "always";
  RUST_BACKTRACE = "full";
}
