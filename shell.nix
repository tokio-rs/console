scope@{ pkgs ? import <nixpkgs> { } }:
with pkgs;
let
  tokio-console = import ./default.nix { inherit pkgs; };

  haveGlibcLocales = pkgs.glibcLocales != null && stdenv.hostPlatform.libc == "glibc";
  glibcLocales-utf8 = (glibcLocales.override { locales = [ "en_US.UTF-8" ]; });

  env = buildEnv {
    name = "console-env";
    paths = [
      direnv
      binutils
      stdenv
      bashInteractive
      docker
      cacert
      gcc
      cmake
      pkg-config
      openssl
    ]
    ++ lib.optional haveGlibcLocales glibcLocales-utf8
    ++ lib.optionals stdenv.isDarwin [ libiconv ]
    ++ tokio-console.buildInputs
    ++ tokio-console.nativeBuildInputs;
  };
in
mkShell {
  buildInputs = [ env ];

  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";
  LOCALE_ARCHIVE = lib.optionalString haveGlibcLocales "${glibcLocales-utf8}/lib/locale/locale-archive";
  LC_ALL = "en_US.UTF-8";
  OPENSSL_DIR = "${openssl.dev}";
  OPENSSL_LIB_DIR = "${openssl.out}/lib";
  SSL_CERT_FILE = "${cacert}/etc/ssl/certs/ca-bundle.crt";
  GIT_SSL_CAINFO = "${cacert}/etc/ssl/certs/ca-bundle.crt";
  CURL_CA_BUNDLE = "${cacert}/etc/ca-bundle.crt";
  CARGO_TERM_COLOR = "always";
  RUST_BACKTRACE = "full";
}
