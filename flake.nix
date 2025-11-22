{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      # support for non-default platforms is best-effort
      systems = inputs.nixpkgs.lib.systems.flakeExposed;
      perSystem = {
        lib,
        pkgs,
        self',
        ...
      }: let
        inherit (pkgs.stdenv.hostPlatform) isDarwin isLinux;

        rust-bin = inputs.rust-overlay.lib.mkRustBin {} pkgs;
        toolchain = rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchain;

        mkArgs = overlay:
          lib.fix (lib.extends (lib.toExtension overlay) (_: {
            src = lib.fileset.toSource rec {
              root = ./.;
              fileset = lib.fileset.unions [
                (craneLib.fileset.commonCargoSources root)
                (lib.fileset.fileFilter (file: file.hasExt "sql") root)
                (lib.fileset.maybeMissing ./assets)
              ];
            };
            nativeBuildInputs = [pkgs.pkg-config];
            buildInputs = lib.flatten [
              (lib.optionals isLinux [
                pkgs.libxkbcommon
                pkgs.xorg.libxcb
                pkgs.xorg.libX11
                (pkgs.alsa-lib-with-plugins.override {
                  plugins = [pkgs.alsa-plugins pkgs.pipewire];
                })
              ])
              (lib.optionals isDarwin [
                pkgs.apple-sdk_15
                (pkgs.darwinMinVersionHook "10.15")
              ])
            ];
            cargoExtraArgs = "--features=hummingbird/runtime_shaders";
          }));
        craneArgs = mkArgs (prev: {cargoArtifacts = craneLib.buildDepsOnly prev;});
      in {
        formatter = pkgs.alejandra;
        apps = builtins.mapAttrs (_: pkg: {program = pkg + /bin/hummingbird;}) self'.packages;
        packages.default = craneLib.buildPackage (mkArgs (prev: {
          CARGO_PROFILE = "release-distro";
          nativeBuildInputs =
            prev.nativeBuildInputs
            ++ lib.optionals isLinux [pkgs.autoPatchelfHook];
          runtimeDependencies = lib.optionals isLinux [
            pkgs.wayland
            pkgs.vulkan-loader
          ];
        }));

        checks = lib.mergeAttrs self'.packages {
          cargoClippy = craneLib.cargoClippy craneArgs;
          cargoTarpaulin = craneLib.cargoTarpaulin craneArgs;
        };

        devShells.default = let
          adapters = lib.flatten [
            (lib.optional isLinux pkgs.stdenvAdapters.useMoldLinker)
          ];
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (rust-bin.selectLatestNightlyWith (toolchain:
            toolchain.default.override {
              extensions = ["rust-analyzer" "rust-src" "clippy" "rustfmt"];
            }));
          craneDevShell = craneLib.devShell.override {
            mkShell = pkgs.mkShell.override {
              stdenv = builtins.foldl' (acc: adapter: adapter acc) pkgs.llvmPackages_latest.stdenv adapters;
            };
          };
        in
          craneDevShell {
            inherit (self') checks;
            packages = [
              pkgs.sqlite-interactive
              pkgs.tokio-console
            ];

            LD_LIBRARY_PATH = lib.optionalString isLinux (
              lib.makeLibraryPath [
                pkgs.vulkan-loader
                pkgs.wayland
              ]
            );

            shellHook = ''
              (
                set -x
                rustc -Vv
                clang -v
              )
            '';
          };
      };
    };
}
