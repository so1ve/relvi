# Relvi

Relvi is a small Wayland application launcher. It keeps an application catalog in memory, searches names, keywords and desktop IDs with [polysearch](https://github.com/so1ve/polysearch), and learns which apps you choose for a query.

## Install

### From source

Build with nightly Rust and the GTK 4, gtk4-layer-shell, libadwaita, and pkg-config development packages installed:

```sh
cargo build --locked --release
install -Dm755 target/release/relvi "$HOME/.local/bin/relvi"
install -Dm644 data/dev.so1ve.Relvi.desktop "$HOME/.local/share/applications/dev.so1ve.Relvi.desktop"
```

Ensure `~/.local/bin` is on the graphical session's `PATH`. The released Linux binaries also need GTK 4, gtk4-layer-shell, and libadwaita at runtime; if those system libraries are unavailable or incompatible, build locally.

### Nix

Run directly:

```sh
nix run github:so1ve/relvi
```

Add the flake to your configuration:

```nix
inputs.relvi.url = "github:so1ve/relvi";
```

Then install its package:

```nix
{ inputs, pkgs, ... }:

{
  environment.systemPackages = [
    inputs.relvi.packages.${pkgs.stdenv.hostPlatform.system}.default
  ];
}
```

## Use

Run `relvi daemon` once after the Wayland session starts. Then bind `relvi toggle` to a compositor shortcut.

For example, add one of the following to your compositor configuration (adjust `Mod`/`SUPER` to your preferred shortcut):

**Niri** (`config.kdl`):

```kdl
spawn-at-startup "relvi" "daemon"

binds {
    Mod+Space { spawn "relvi" "toggle"; }
}
```

**Hyprland**

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("relvi daemon")
end)

hl.bind("SUPER + SPACE", hl.dsp.exec_cmd("relvi toggle"))
```

**Sway** (`config`):

```text
exec relvi daemon
bindsym $mod+space exec relvi toggle
```

Run `relvi` to show the launcher directly, `relvi quit` to stop the resident process, or `relvi clear-history` to erase launch and query history. `Enter` or a single click opens the selected app. Use the arrow keys or `Ctrl+J/K` and `Ctrl+N/P` to change the selection.

Category tabs filter the current search. Scroll over the category bar to browse the tabs. Use `Ctrl+H` / `Ctrl+L` or `Shift+Tab` / `Tab` to select the previous / next category while keeping the search field focused. `Escape` hides the launcher.

## I18N

By default, Reslvi supports English and Chinese Pinyin as search input. This ability is provided by [polysearch](https://github.com/so1ve/polysearch). Create a feature request or PR there if you want to add more languages.

## LICENSE

[MIT](LICENSE). Made with ❤️ by [Ray](https://github.com/so1ve)
