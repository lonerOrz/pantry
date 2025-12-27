{
  description = "A generic selector for various types of entries";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      perSystem =
        {
          config,
          self',
          inputs',
          pkgs,
          system,
          ...
        }:
        let
          lib = pkgs.lib;

          # GTK4 and related dependencies
          buildInputs =
            with pkgs;
            [
              gtk4
              glib
              graphene
              gdk-pixbuf
              cairo
              pango
              harfbuzz
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              # macOS-specific dependencies
              libiconv
            ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        in
        {
          packages = {
            default = self'.packages.pantry;
            pantry = pkgs.rustPlatform.buildRustPackage {
              pname = "pantry";
              version = "0.1.0";
              src = ./.;
              cargoLock = {
                lockFile = ./Cargo.lock;
              };
              inherit buildInputs nativeBuildInputs;
              meta = with lib; {
                description = "A generic selector for various types of entries";
                homepage = "https://github.com/lonerOrz/pantry";
                license = licenses.mit;
                mainProgram = "pantry";
                maintainers = with lib.maintainers; [ lonerOrz ];
                platforms = [
                  "x86_64-linux"
                  "aarch64-linux"
                  "x86_64-darwin"
                  "aarch64-darwin"
                ];
              };
            };
          };

          devShells.default = pkgs.mkShell {
            inherit buildInputs nativeBuildInputs;
            packages = with pkgs; [
              rustc
              cargo
              rust-analyzer
              rustfmt
              clippy
            ];

            env = {
              LD_LIBRARY_PATH = lib.optionalString (
                !pkgs.stdenv.isDarwin
              ) "${pkgs.lib.makeLibraryPath buildInputs}";
              DYLD_LIBRARY_PATH = lib.optionalString pkgs.stdenv.isDarwin "${pkgs.lib.makeLibraryPath buildInputs}";
            };
          };

          treefmt = {
            projectRootFile = "flake.nix";
            programs.nixfmt = {
              enable = true;
              package = pkgs.nixfmt-rfc-style;
            };
            programs.rustfmt.enable = true;
          };
        };
    };
}
