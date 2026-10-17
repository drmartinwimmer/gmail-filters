{
  description = "Agentic skills, Rust CLI tools, and gmailctl configuration for reviewing and updating Gmail filters";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
        gmailFiltersPackage = rustPlatform.buildRustPackage {
          pname = "gmail-filters";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              toolchain
              (cargo-audit.override { inherit rustPlatform; })
              gmailctl
              taplo
              prettier
            ];
          };

        packages = {
          default = gmailFiltersPackage;
          gmail-filters = gmailFiltersPackage;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = gmailFiltersPackage;
          };
          gmail-filters = flake-utils.lib.mkApp {
            drv = gmailFiltersPackage;
          };
        };
      }
    );
}
