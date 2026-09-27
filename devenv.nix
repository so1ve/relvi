{ inputs, lib, pkgs, ... }:
{
  imports = [
    (inputs.ray-devenv + "/profiles/config.nix")
    (inputs.ray-devenv + "/profiles/rust.nix")
  ]
  ++ lib.optional (builtins.pathExists ./devenv.local.nix) ./devenv.local.nix;

  packages = with pkgs; [
    gtk4
    gtk4-layer-shell
    libadwaita
    pkg-config
  ];
}
