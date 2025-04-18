{
  pkgs ? import <nixpkgs> { },
}:
let
  alsaConfPath = "${pkgs.alsa-lib}/share/alsa/alsa.conf";

  dependencies = with pkgs; [
    # Required system libraries
    clang
    pkg-config

    # For audio & MIDI
    alsa-lib
    alsa-plugins
    alsa-utils
    pulseaudio

    # For GUI
    xorg.libX11
    xorg.libXcursor
    xorg.libXrandr
    xorg.libXi
    xorg.libXinerama
    xorg.libXext
    xorg.libXft
    xorg.libXtst
    xorg.libXrender
    xorg.libXcomposite
    xorg.libXdamage
    xorg.libXfixes
    xorg.libxcb

    # Keyboard handling
    libxkbcommon
    xorg.libxkbfile

    # For Vulkan support
    vulkan-loader
    vulkan-tools
    vulkan-headers
    vulkan-validation-layers

    # Other graphics libraries
    libGL
    libGLU
    libglvnd
    linuxPackages.nvidia_x11
  ];
in
pkgs.mkShell {
  buildInputs = dependencies;

  shellHook = ''
    # Set up library paths
    export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath dependencies}"

    # Set up ALSA configuration
    export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"
    export ALSA_CONFIG_PATH="${alsaConfPath}"

    # Set up X11 configuration
    export XCURSOR_PATH="${pkgs.xorg.xcursorthemes}/share/icons"

    # Set up Vulkan configuration
    export VK_LAYER_PATH="${pkgs.vulkan-validation-layers}/share/vulkan/explicit_layer.d"
    # Find and set up Vulkan ICD files
    VULKAN_ICDS=""

    # Add Lavapipe software renderer
    if [ -f "${pkgs.vulkan-loader}/share/vulkan/icd.d/lvp_icd.x86_64.json" ]; then
      VULKAN_ICDS="$VULKAN_ICDS:${pkgs.vulkan-loader}/share/vulkan/icd.d/lvp_icd.x86_64.json"
    fi

    # Find NVIDIA 64-bit ICD
    NVIDIA_ICD_64=$(find /nix/store -path "*/share/vulkan/icd.d/nvidia_icd.x86_64.json" | head -n 1)
    if [ -n "$NVIDIA_ICD_64" ]; then
      VULKAN_ICDS="$VULKAN_ICDS:$NVIDIA_ICD_64"
      echo "Found NVIDIA 64-bit Vulkan ICD: $NVIDIA_ICD_64"
    fi

    # Set Vulkan ICDs
    if [ -n "$VULKAN_ICDS" ]; then
      # Remove leading colon if present
      VULKAN_ICDS=$(echo "$VULKAN_ICDS" | sed 's/^://')
      export VK_ICD_FILENAMES="$VULKAN_ICDS"
      echo "VK_ICD_FILENAMES set to: $VK_ICD_FILENAMES"
    else
      echo "Warning: No Vulkan ICDs found"
    fi

    # Set up OpenGL/EGL configuration
    export __EGL_VENDOR_LIBRARY_DIRS="${pkgs.libglvnd}/share/glvnd/egl_vendor.d"
    export __GLX_VENDOR_LIBRARY_NAME=mesa

    # Force software rendering for OpenGL
    export LIBGL_ALWAYS_SOFTWARE=1
    export GALLIUM_DRIVER=llvmpipe

    # Force WGPU to use OpenGL instead of Vulkan
    export WGPU_BACKEND=gl

    # Create a custom ALSA config with the correct paths
    mkdir -p ~/.config/alsa
    cat > ~/.config/alsa/asoundrc << EOF
    pcm.!default {
      type pulse
      fallback "sysdefault"
      hint {
        show on
        description "Default ALSA Output (PulseAudio)"
      }
    }

    ctl.!default {
      type pulse
      fallback "sysdefault"
    }

    # MIDI configuration
    seq.default {
      type hw
    }

    seq.hw {
      type hw
    }
    EOF

    # Print debug information
    echo "LD_LIBRARY_PATH: $LD_LIBRARY_PATH"
    echo "VK_LAYER_PATH: $VK_LAYER_PATH"
    echo "VK_ICD_FILENAMES: $VK_ICD_FILENAMES"
  '';
}
