# Development shell for the mapserver-rs workspace.
#
# Provides:
#   - libmapserver + headers (pkgs.mapserver)
#   - libclang                 (for bindgen code generation)
#   - glibc dev headers        (so bindgen can parse <stdio.h> etc.)
#
# Run with:
#   nix-shell            (uses <nixpkgs> from NIX_PATH)
#   nix-shell shell.nix
#
# The build.rs in mapserver-sys runs bindgen against
# `mapserver/mapserver.h`, which transitively includes headers from
# proj, gdal, freetype, fribidi, harfbuzz, libpq, libxml2, ... so we
# expose every header directory that libmapserver itself was built
# against via BINDGEN_EXTRA_CLANG_ARGS.
{ pkgs ? import <nixpkgs> {} }:

let
  inherit (pkgs) lib;

  # Python mapscript is not needed for the Rust bindings.
  mapserver = pkgs.mapserver.override { withPython = false; };

  # Header include dirs needed to parse mapserver/mapserver.h.
  headerDeps = mapserver.buildInputs ++ [ mapserver pkgs.glibc.dev ];

  includeFlags = lib.concatMapStringsSep " "
    (p: "-isystem ${lib.getInclude p}/include")
    headerDeps;

  clangInclude =
    "${pkgs.libclang.lib}/lib/clang/${pkgs.libclang.version}/include";
in

pkgs.mkShell {
  name = "mapserver-rs";

  buildInputs = [ mapserver pkgs.libclang ] ++ mapserver.buildInputs;

  # bindgen locates libclang.so through this variable.
  LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

  # Extra -isystem paths consumed by bindgen's clang invocation.
  # mapserver.h is a C header; parse wrapper.hpp as C so bindgen does not
  # pull in the C++ standard library (gdal's cpl_port.h would otherwise
  # #include <cmath> etc.).
  BINDGEN_EXTRA_CLANG_ARGS =
    "-x c ${includeFlags} -isystem ${clangInclude}";

  # Locate libmapserver.so when running built binaries / tests.
  LD_LIBRARY_PATH = "${mapserver}/lib";
}
