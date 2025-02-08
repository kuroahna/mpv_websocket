{
  description = "mpv WebSocket";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      flake-utils,
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        toolchain = p: p.rust-bin.stable.latest.default;
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        mpv_websocket = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );

        crossCompileForWindowsX64 =
          let
            targetTriple = "x86_64-pc-windows-gnu";

            # Hack required to fix link errors with pthreads on stable 1.84.0
            # This does not seem to be required on nightly 1.86.0
            #
            # https://github.com/nix-community/naersk/issues/181#issuecomment-874352470
            fixLinkErrors = ''
              export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-args=''$(echo $NIX_LDFLAGS | tr ' ' '\n' | grep -- '^-L' | tr '\n' ' ')"
              export NIX_LDFLAGS=
            '';

            toolchain =
              p:
              p.rust-bin.stable.latest.default.override {
                targets = [ targetTriple ];
              };
            craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

            src = craneLib.cleanCargoSource ./.;

            commonArgs = {
              inherit src;
              strictDeps = true;
            };

            cargoArtifacts = craneLib.buildDepsOnly (
              commonArgs
              // {
                nativeBuildInputs = with pkgs; [
                  pkgsCross.mingwW64.stdenv.cc
                ];
                buildInputs = with pkgs; [
                  pkgsCross.mingwW64.windows.mingw_w64_pthreads
                ];

                preBuild = fixLinkErrors;

                buildPhaseCargoCommand = ''
                  cargo check --profile release --frozen --target ${targetTriple}
                  cargo build --profile release --frozen --target ${targetTriple} --workspace
                '';
                checkPhaseCargoCommand = ''
                  cargo test --profile release --frozen --target ${targetTriple} --workspace --no-run
                '';
              }
            );

            mpv_websocket = craneLib.buildPackage (
              commonArgs
              // {
                inherit cargoArtifacts;
                nativeBuildInputs = with pkgs; [
                  pkgsCross.mingwW64.stdenv.cc
                  wine64
                ];
                buildInputs = with pkgs; [
                  pkgsCross.mingwW64.windows.mingw_w64_pthreads
                ];

                preConfigure = ''
                  # Required for wine
                  export HOME=$(mktemp --directory)
                '';

                preBuild = fixLinkErrors;

                cargoExtraArgs = "--frozen --target ${targetTriple} --workspace";
                CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER = "wine64";
              }
            );
          in
          {
            src = src;
            commonArgs = commonArgs;
            craneLib = craneLib;
            cargoArtifacts = cargoArtifacts;
            mpv_websocket = mpv_websocket;
            targetTriple = targetTriple;
          };
      in
      {
        formatter = pkgs.nixfmt-rfc-style;
        packages.default = mpv_websocket;
        packages.windowsX64 = crossCompileForWindowsX64.mpv_websocket;
        checks = {
          inherit mpv_websocket;

          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          format = craneLib.cargoFmt {
            inherit src;
          };

          toml_format = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };

          deny = craneLib.cargoDeny { inherit src; };
        };
        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs; [
            rust-analyzer
          ];

          # fixes: the cargo feature `public-dependency` requires a nightly
          # version of Cargo, but this is the `stable` channel
          #
          # This enables unstable features with the stable compiler
          # Remove once this is fixed in stable
          #
          # https://github.com/rust-lang/rust/issues/112391
          # https://github.com/rust-lang/rust-analyzer/issues/15046
          RUSTC_BOOTSTRAP = 1;
        };
      }
    );
}
