# flake.nix
{
  description = "Development environment for a Rust MIDI Sequencer with Iced GUI";

  # Define the inputs for this flake, primarily nixpkgs and helpers
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable"; # Or use a specific stable release if preferred
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay"; # For easy Rust toolchain management
  };

  # Define the outputs provided by this flake
  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    # Use flake-utils to easily support common systems
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        # Import nixpkgs for the specific system with the Rust overlay applied
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # === Rust Toolchain Configuration ===
        # Select your desired Rust toolchain (e.g., stable, nightly, specific version)
        rustToolchain = pkgs.rust-bin.stable.latest.default;
        # Or use nightly:
        # rustToolchain = pkgs.rust-bin.nightly.latest.default;

        # === System Dependencies ===
        # Map the required .so files to Nix packages and add common GUI deps
        nativeDeps = with pkgs; [
          # Dependencies directly from ldd output:
          alsa-lib # Provides libasound.so.2 (for MIDI/Audio)
          alsa-plugins
          alsa-utils
          xorg.libX11 # Provides libX11.so.6
          # libgcc_s.so.1, libm.so.6, libc.so.6, ld-linux are usually part of stdenv/glibc
          xorg.libxcb # Provides libxcb.so.1
          xorg.libXau # Provides libXau.so.6
          xorg.libXdmcp # Provides libXdmcp.so.6
          libbsd # Provides libbsd.so.0 (and often libmd.so.0)

          # Common dependencies for GUI toolkits like Iced (using wgpu/vulkan/opengl):
          pkg-config # Often needed by build scripts to find libraries
          xorg.libXcursor # For mouse cursor handling
          xorg.libXrandr # For display management
          xorg.libXi # For input devices extension
          libxkbcommon # For keyboard input handling
          fontconfig # For font management
          freetype # Font rendering
          dbus # For inter-process communication (common in desktop apps)
          wayland # For Wayland display server support
          libglvnd # OpenGL dispatch library (if using OpenGL backend)
          mesa # If using OpenGL directly / drivers
          vulkan-loader # If using Vulkan backend (wgpu often prefers this)
          vulkan-tools # Optional: for vulkan info/debugging
          libdrm # Direct Rendering Manager library

          # Build-time C/C++ compiler if needed by build scripts (e.g., crates binding C libs)
          gcc
          clang # Some crates might prefer clang
        ];

      in
      {
        # Define the default development shell activated by `nix develop`
        devShells.default = pkgs.mkShell {
          # Tools and libraries available in the shell
          packages = [
            rustToolchain # The selected Rust toolchain (includes rustc, cargo, etc.)
            pkgs.cargo-watch # Optional: useful for auto-recompiling
            pkgs.rust-analyzer # For IDE support (LSP)

            # Add the system dependencies
          ] ++ nativeDeps;

          # Environment variables to set within the shell
          shellHook = ''
                        # Optional: Inform the user what's happening
                        echo "Entering Rust MIDI Sequencer dev environment..."

                        # Set RUST_SRC_PATH for rust-analyzer to find standard library sources
                        # Adjust the path if you chose a different toolchain than nightly
                        export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
            export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"

                        # Optional: Add target/debug and target/release to PATH for easy execution
                        # export PATH=$PWD/target/debug:$PWD/target/release:$PATH
          '';

          PKG_CONFIG_PATH = "${pkgs.alsa-lib}/lib/pkgconfig";
        };
      }
    );
}
