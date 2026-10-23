# Development shell for the mapserver-rs workspace.
# Provides libmapserver, libclang, and glibc dev headers
{
  pkgs ? import <nixpkgs> { },
}:

let
  inherit (pkgs) lib;

  mapserver = pkgs.mapserver.override { withPython = false; };

  headerDeps = mapserver.buildInputs ++ [
    mapserver
    pkgs.glibc.dev
  ];

  includeFlags = lib.concatMapStringsSep " " (p: "-isystem ${lib.getInclude p}/include") headerDeps;

  clangInclude = "${pkgs.libclang.lib}/lib/clang/${pkgs.libclang.version}/include";
in

pkgs.mkShell {
  name = "mapserver-rs";

  buildInputs = [
    mapserver
    pkgs.libclang
  ]
  ++ mapserver.buildInputs;

  # bindgen locates libclang.so through this variable.
  LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

  # build.rs in mapserver-sys runs bindgen against
  # `mapserver/mapserver.h`, which transitively includes headers from
  # proj, gdal, freetype, fribidi, harfbuzz, libpq, libxml2, ... so we
  # expose every header directory that libmapserver itself was built
  # against via BINDGEN_EXTRA_CLANG_ARGS.
  BINDGEN_EXTRA_CLANG_ARGS = "-x c ${includeFlags} -isystem ${clangInclude}";

  LD_LIBRARY_PATH = "${mapserver}/lib";
}
