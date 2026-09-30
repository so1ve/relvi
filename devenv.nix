{ pkgs, ... }:

{
  languages.rust = {
    enable = true;
    toolchainFile = ./rust-toolchain.toml;
  };

  packages = with pkgs; [
    actionlint
    gtk4
    gtk4-layer-shell
    libadwaita
    nixfmt-tree
    pkg-config
    tombi
  ];
}
