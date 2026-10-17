{
  description = "Agentic skills, Rust CLI tools (gmail-triage), and gmailctl configuration for reviewing and updating Gmail filters";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    let
      hmModule =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.programs.gmail-triage;
        in
        {
          options.programs.gmail-triage = {
            enable = lib.mkEnableOption "gmail-triage CLI and unified gmailctl configuration directory";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.gmail-triage;
              description = "The gmail-triage package to install.";
            };

            configDir = lib.mkOption {
              type = lib.types.str;
              default = "${config.xdg.configHome}/gmail-triage";
              description = "Unified directory containing config.jsonnet and gmail-triage.toml.";
            };

            configJsonnet = lib.mkOption {
              type = lib.types.nullOr lib.types.lines;
              default = null;
              description = "Optional declarative config.jsonnet contents written into configDir.";
            };

            minConfidenceBps = lib.mkOption {
              type = lib.types.int;
              default = 7500;
              description = "Minimum classifier confidence in basis points (0..=10000).";
            };
          };

          config = lib.mkIf cfg.enable {
            home.packages = [
              cfg.package
              pkgs.gmailctl
            ];

            home.sessionVariables = {
              GMAIL_TRIAGE_CONFIG_DIR = cfg.configDir;
            };

            xdg.configFile = lib.mkMerge [
              {
                "gmail-triage/gmail-triage.toml".text = ''
                  min_confidence_bps = ${toString cfg.minConfidenceBps}
                '';
              }
              (lib.mkIf (cfg.configJsonnet != null) {
                "gmail-triage/config.jsonnet".text = cfg.configJsonnet;
              })
            ];
          };
        };
    in
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
        gmailTriagePackage = rustPlatform.buildRustPackage {
          pname = "gmail-triage";
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
          default = gmailTriagePackage;
          gmail-triage = gmailTriagePackage;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            drv = gmailTriagePackage;
          };
          gmail-triage = flake-utils.lib.mkApp {
            drv = gmailTriagePackage;
          };
        };
      }
    )
    // {
      homeManagerModules.default = hmModule;
      homeManagerModules.gmail-triage = hmModule;
    };
}
