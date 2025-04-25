{
  description = "Sequencer development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;
        nativeDeps = with pkgs; [
          alsa-lib
          alsa-plugins
          alsa-utils
          xorg.libX11
          xorg.libxcb
          xorg.libXau
          xorg.libXdmcp
          libbsd

          pkg-config
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          libxkbcommon
          fontconfig
          freetype
          dbus
          wayland
          libglvnd
          mesa
          vulkan-loader
          vulkan-tools
          libdrm

          gcc
          clang
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.cargo-watch
            pkgs.rust-analyzer

          ] ++ nativeDeps;

          shellHook = ''
            # Set RUST_SRC_PATH for rust-analyzer to find standard library sources
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"

            # Needed to find all the alsa .so files
            export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"

            # This one is needed so that libxkbcommon-x11.so is linked correctly
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath nativeDeps}:$LD_LIBRARY_PATH"
          '';

          PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "lib" "pkgconfig" [
            pkgs.alsa-lib
            pkgs.libxkbcommon
            pkgs.xorg.libX11
            pkgs.xorg.libxcb
          ];
        };
      }
    );
}
