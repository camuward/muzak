{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    contemporary-rs = {
      url = "github:vicr123/contemporary-rs/v0.1.0";
      flake = false;
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      # support for non-default platforms is best-effort
      systems = inputs.nixpkgs.lib.systems.flakeExposed;
      perSystem = {
        lib,
        self',
        system,
        ...
      }: let
        inherit (pkgs.stdenv.hostPlatform) isDarwin isLinux;
        pkgs = import inputs.nixpkgs {
          inherit system;
          config.allowDeprecatedx86_64Darwin = true;
        };

        rust-bin = inputs.rust-overlay.lib.mkRustBin {} pkgs;
        craneLib = let
          toolchain = rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
          ((inputs.crane.mkLib pkgs).overrideToolchain toolchain).overrideScope (_: _: {
            stdenvSelector = p:
              builtins.foldl' (acc: adapter: adapter acc) p.llvmPackages_latest.libcxxStdenv (lib.flatten [
                (lib.optional p.stdenv.hostPlatform.isLinux p.stdenvAdapters.useMoldLinker)
                (p.stdenvAdapters.withCFlags ["-flto=thin" "-Os"])
              ]);
          });

        mkArgs = overlay:
          lib.fix (lib.extends (lib.toExtension overlay) (_: {
            src = lib.fileset.toSource rec {
              root = ./.;
              fileset = lib.fileset.unions [
                (craneLib.fileset.commonCargoSources root)
                (lib.fileset.fileFilter (file: file.hasExt "sql") root)
                ./Contemporary.toml
                ./assets
                ./res
                ./translations
              ];
            };

            nativeBuildInputs = [pkgs.cmake pkgs.pkg-config];
            buildInputs = lib.flatten [
              (lib.optionals isLinux [
                pkgs.libxkbcommon
                pkgs.libxcb
                pkgs.libX11
                pkgs.fontconfig
                (pkgs.alsa-lib-with-plugins.override {
                  plugins = [pkgs.alsa-plugins pkgs.pipewire];
                })
              ])
              (lib.optionals isDarwin [
                pkgs.apple-sdk_15
                (pkgs.darwinMinVersionHook "10.15")
              ])
            ];

            CC_ENABLE_DEBUG_OUTPUT = "1";
            cargoExtraArgs = "--features=hummingbird/runtime_shaders -vv";

            HUMMINGBIRD_VERSION_ID = builtins.substring 0 7 (inputs.self.rev or "dirty");
            HUMMINGBIRD_RELEASE_CHANNEL = "flake";
          }));
        craneArgs = mkArgs (prev: {cargoArtifacts = craneLib.buildDepsOnly prev;});
      in {
        formatter = pkgs.alejandra;
        apps = builtins.mapAttrs (_: pkg: {program = pkg + /bin/hummingbird;}) self'.packages;
        packages.default = craneLib.buildPackage (mkArgs (prev: {
          nativeBuildInputs =
            prev.nativeBuildInputs
            ++ [pkgs.llvmPackages_latest.llvm pkgs.llvmPackages_latest.lld]
            ++ [
              (craneLib.buildPackage rec {
                src = inputs.contemporary-rs;
                inherit (craneLib.crateNameFromCargoToml {cargoToml = src + /deploy_tool/cargo_cntp_bundle/Cargo.toml;}) pname version;
                nativeBuildInputs = [pkgs.perl];
                cargoExtraArgs = "-p cargo-cntp-bundle";
              })
            ]
            ++ lib.optionals isDarwin [
              (pkgs.runCommandLocal "iconutil-shim" {nativeBuildInputs = [pkgs.makeWrapper];} ''
                makeWrapper ${lib.getExe' pkgs.libicns "icnsutil"} "$out/bin/iconutil"
              '')
              # https://github.com/rust-lang/rust/issues/60059#issuecomment-1972748340
              (pkgs.writeShellApplication {
                name = "macos-linker";
                text = ''
                  declare -a args=()
                  for arg in "$@"
                  do
                    # options for linker
                    if [[ $arg == "-Wl,"* ]]; then
                      IFS=',' read -r -a options <<< "''${arg#-Wl,}"
                      for option in "''${options[@]}"
                      do
                        if [[ $option == "-plugin="* ]] || [[ $option == "-plugin-opt=mcpu="* ]]; then
                          # ignore -lto_library and -plugin-opt=mcpu
                          :
                        elif [[ $option == "-plugin-opt=O"* ]]; then
                          # convert -plugin-opt=O* to --lto-CGO*
                          args[''${#args[@]}]="-Wl,--lto-CGO''${option#-plugin-opt=O}"
                        else
                          # pass through other arguments
                          args[''${#args[@]}]="-Wl,$option"
                        fi
                      done

                    else
                      # pass through other arguments
                      args[''${#args[@]}]="$arg"
                    fi
                  done

                  # use clang to call ld64
                  exec ''${CC} -v "''${args[@]}"
                '';
              })
            ]
            ++ lib.optionals isLinux [pkgs.autoPatchelfHook];
          runtimeDependencies = lib.optionals isLinux [
            pkgs.wayland
            pkgs.vulkan-loader
          ];

          CARGO_PROFILE = "release-debug";
          CARGO_BUILD_RUSTFLAGS = lib.concatStringsSep " " [
            "-Csymbol-mangling-version=v0"
            "-Clinker-plugin-lto"
            "-Clinker=${
              if isDarwin
              then "macos-linker"
              else "clang"
            }"
            "-Clink-arg=--ld-path=${
              if isDarwin
              then "ld64.lld"
              else "ld.lld"
            }"
          ];

          dontStrip = true;
          installPhaseCommand =
            ''
              (
                OUTPUT_BIN="''${CARGO_TARGET_DIR:-target}"/"$CARGO_PROFILE"/hummingbird
                llvm-dwarfutil "$OUTPUT_BIN" "$OUTPUT_BIN"
                llvm-objcopy --compress-debug-sections=zlib "$OUTPUT_BIN"
              )
            ''
            + ''
              cargo cntp-bundle --no-open --profile "$CARGO_PROFILE"
            ''
            + lib.optionalString isLinux ''
              cp -a "''${CARGO_TARGET_DIR:-target}"/bundle/*/"$CARGO_PROFILE"/appdir/usr/. "$out"
            ''
            + lib.optionalString isDarwin ''
              mkdir -p "$out/Applications"
              cp -a "''${CARGO_TARGET_DIR:-target}"/bundle/*/"$CARGO_PROFILE"/Hummingbird.app "$out/Applications"
              mkdir -p "$out/bin"
              ln -s "$out/Applications/Hummingbird.app/Contents/MacOS/hummingbird" "$out/bin/hummingbird"
            '';
        }));

        checks = lib.mergeAttrs self'.packages {
          cargoClippy = craneLib.cargoClippy craneArgs;
        };

        devShells.default = let
          nightlyCraneLib = craneLib.overrideToolchain (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {extensions = ["rust-analyzer" "rust-src" "clippy" "rustfmt" "rustc-codegen-cranelift-preview"];}));
        in
          nightlyCraneLib.devShell {
            inherit (self') checks;
            packages =
              [
                pkgs.sqlite-interactive
                pkgs.tokio-console
              ]
              ++ lib.optionals isLinux [
                pkgs.mold
                pkgs.wild
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
