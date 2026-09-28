{
  description = "A small Wayland application launcher";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    polysearch = {
      url = "git+file:///home/ray/Develop/polysearch";
      flake = false;
    };
  };

  outputs =
    { nixpkgs, polysearch, ... }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
    in
    {
      packages = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          relvi = pkgs.rustPlatform.buildRustPackage {
            pname = cargoToml.package.name;
            inherit (cargoToml.package) version;

            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./src
                ./resources
                ./data
              ];
            };

            cargoLock.lockFile = ./Cargo.lock;

            # Cargo.toml refers to the sibling ../polysearch directory.
            postUnpack = ''
              cp -R --no-preserve=mode ${polysearch} polysearch
            '';

            nativeBuildInputs = with pkgs; [
              pkg-config
              wrapGAppsHook4
            ];

            buildInputs = with pkgs; [
              gtk4
              gtk4-layer-shell
              libadwaita
            ];

            postInstall = ''
              install -Dm644 data/dev.so1ve.Relvi.desktop \
                "$out/share/applications/dev.so1ve.Relvi.desktop"
            '';

            meta = {
              description = "A small Wayland application launcher";
              license = pkgs.lib.licenses.mit;
              mainProgram = "relvi";
              platforms = pkgs.lib.platforms.linux;
            };
          };
        in
        {
          inherit relvi;
          default = relvi;
        }
      );
    };
}
