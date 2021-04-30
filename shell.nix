scope@{ pkgs ? import <nixpkgs> { } }:

let env = (import ./default.nix scope);

in with pkgs;
mkShell {
  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";
  LOCALE_ARCHIVE = "${glibcLocales}/lib/locale/locale-archive";
  LC_ALL = "en_US.UTF-8";
  OPENSSL_DIR = "${openssl.dev}";
  OPENSSL_LIB_DIR = "${openssl.out}/lib";
  SSL_CERT_FILE = "${cacert}/etc/ssl/certs/ca-bundle.crt";
  GIT_SSL_CAINFO = "${cacert}/etc/ssl/certs/ca-bundle.crt";
  CURL_CA_BUNDLE = "${cacert}/etc/ca-bundle.crt";
  CARGO_TERM_COLOR = "always";
  RUST_BACKTRACE = "1";
  buildInputs = [ (import ./default.nix { inherit pkgs; }) ];
}
