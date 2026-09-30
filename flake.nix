{
  description = "A focused launcher for Wayland";

  nixConfig = {
    extra-substituters = [ "https://so1ve.cachix.org" ];
    extra-trusted-public-keys = [
      "so1ve.cachix.org-1:51jcW4FkJhiLcqPsiUx3nglRP469les8F9zjFxio1nw="
    ];
  };

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      mkPackage =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          inherit (cargoToml.package) version;

          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.lock
              ./Cargo.toml
              ./src
              ./xtask
              ./resources
              ./data
            ];
          };

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            installShellFiles
            pkg-config
            wrapGAppsHook4
          ];

          buildInputs = with pkgs; [
            gtk4
            gtk4-layer-shell
            libadwaita
          ];

          preFixup = ''
            gappsWrapperArgs+=(--prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.wtype ]})
          '';

          postInstall = ''
            install -Dm644 data/dev.so1ve.Relvi.desktop \
              "$out/share/applications/dev.so1ve.Relvi.desktop"
            install -Dm644 data/dev.so1ve.Relvi.svg \
              "$out/share/icons/hicolor/scalable/apps/dev.so1ve.Relvi.svg"

            installShellCompletion --cmd relvi \
              --bash <("$out/bin/relvi" completions --shell bash) \
              --fish <("$out/bin/relvi" completions --shell fish) \
              --zsh <("$out/bin/relvi" completions --shell zsh)
          '';

          meta = {
            inherit (cargoToml.package) description;
            homepage = cargoToml.package.repository;
            license = pkgs.lib.licenses.mit;
            mainProgram = cargoToml.package.name;
            platforms = pkgs.lib.platforms.linux;
          };
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          relvi = mkPackage pkgs;
        in
        {
          inherit relvi;
          default = relvi;
        }
      );

      apps = forAllSystems (
        system:
        let
          app = {
            type = "app";
            program = nixpkgs.lib.getExe self.packages.${system}.relvi;
            meta.description = cargoToml.package.description;
          };
        in
        {
          relvi = app;
          default = app;
        }
      );

      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt-tree);

      overlays.default = final: _prev: {
        relvi = mkPackage final;
      };
    };
}
