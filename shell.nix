{ pkgs ? import <nixpkgs> {} }:
let
    # rust-rover things
    fenix = import (fetchTarball "https://github.com/nix-community/fenix/archive/main.tar.gz") { };
    rust-toolchain =
        fenix.default.toolchain;
in
pkgs.mkShell rec {
    buildInputs = with pkgs;[
        openssl
        pkg-config
        cmake
        zlib
        rust-toolchain

        dbus

        # common glutin
        libxkbcommon
        libGL

        # video
        glib
        gst_all_1.gstreamer
        gst_all_1.gst-plugins-base

        # winit wayland
        wayland

        # winit x11
        xorg.libXcursor
        xorg.libXrandr
        xorg.libXi
        xorg.libX11
    ];
    nativeBuildInputs = with pkgs; [
        pkg-config
        fontconfig
    ];
    LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath buildInputs}";
    OPENSSL_DIR="${pkgs.openssl.dev}";
    OPENSSL_LIB_DIR="${pkgs.openssl.out}/lib";
    RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    GST_PLUGIN_PATH = "${pkgs.gst_all_1.gstreamer}:${pkgs.gst_all_1.gst-plugins-bad}:${pkgs.gst_all_1.gst-plugins-ugly}:${pkgs.gst_all_1.gst-plugins-good}:${pkgs.gst_all_1.gst-plugins-base}";
}
