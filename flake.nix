{
  description = "A generic selector for various types of entries";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      treefmt-nix,
      ...
    }:
    let
      cargoToml = fromTOML (builtins.readFile ./Cargo.toml);

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          let
            pkgs = import nixpkgs { inherit system; };
            lib = pkgs.lib;
          in
          f {
            inherit system pkgs lib;
          }
        );

      commonDeps =
        { pkgs, lib }:
        {
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
              libiconv
            ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };
    in
    {
      packages = forAllSystems (
        { pkgs, lib, ... }:
        let
          deps = commonDeps { inherit pkgs lib; };

          package = pkgs.rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            version = cargoToml.package.version;

            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            inherit (deps) buildInputs nativeBuildInputs;

            meta = {
              description = "A generic selector for various types of entries";
              homepage = "https://github.com/lonerOrz/pantry";
              license = lib.licenses.mit;
              mainProgram = cargoToml.package.name;
              maintainers = with lib.maintainers; [ lonerOrz ];
              platforms = systems;
            };
          };
        in
        {
          default = package;
          ${cargoToml.package.name} = package;
        }
      );

      devShells = forAllSystems (
        { pkgs, lib, ... }:
        let
          deps = commonDeps { inherit pkgs lib; };
        in
        {
          default = pkgs.mkShell {
            inherit (deps) buildInputs nativeBuildInputs;

            packages = with pkgs; [
              rustc
              cargo
              rust-analyzer
              rustfmt
              clippy
            ];

            LD_LIBRARY_PATH = lib.optionalString (!pkgs.stdenv.isDarwin) (lib.makeLibraryPath deps.buildInputs);

            DYLD_LIBRARY_PATH = lib.optionalString pkgs.stdenv.isDarwin (lib.makeLibraryPath deps.buildInputs);
          };
        }
      );

      formatter = forAllSystems (
        { pkgs, ... }:
        treefmt-nix.lib.mkWrapper pkgs {
          projectRootFile = "flake.nix";

          programs.nixfmt = {
            enable = true;
            package = pkgs.nixfmt-rfc-style;
          };

          programs.rustfmt.enable = true;
        }
      );
    };
}
