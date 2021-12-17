{ pkgs ? import <nixpkgs> { } }:
pkgs.callPackage ./tokio-console.nix { }
