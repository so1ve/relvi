flake:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.relvi;
  unit = "${cfg.package}/lib/systemd/user/relvi.service";
in
{
  options.services.relvi = {
    enable = lib.mkEnableOption "Relvi";

    package = lib.mkOption {
      type = lib.types.package;
      default = flake.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "inputs.relvi.packages.${pkgs.stdenv.hostPlatform.system}.default";
      description = "Relvi package to install and run.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile = {
      "systemd/user/relvi.service".source = unit;
      "systemd/user/graphical-session.target.wants/relvi.service".source = unit;
      "systemd/user/relvi.service.d/override.conf".text = ''
        [Service]
        ExecStop=${lib.getExe cfg.package} quit
        RestartSec=3
        KillMode=process
      '';
    };
  };
}
