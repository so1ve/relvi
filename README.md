<p align="center">
  <img src="data/dev.so1ve.Relvi.svg" width="96" height="96" alt="Relvi logo" />
</p>

<h1 align="center">Relvi</h1>
<h4 align="center">A focused application launcher for Wayland</h4>

Relvi is a lightweight launcher focused on app search. Open Relvi with a shortcut, type what you remember, and press Enter to launch an app.

- Typo-tolerant search, Pinyin, and initials.
- App names take priority over descriptions; history helps rank similar matches.
- Runs in the background and updates the app list automatically.

Search uses [Polysearch](https://github.com/so1ve/polysearch). I tune its matching rules based on everyday use.

## Install

### From source

Build with nightly Rust and the GTK 4, gtk4-layer-shell, libadwaita, and pkg-config development packages installed:

```sh
cargo build --locked --release
install -Dm755 target/release/relvi "$HOME/.local/bin/relvi"
install -Dm644 data/dev.so1ve.Relvi.desktop "$HOME/.local/share/applications/dev.so1ve.Relvi.desktop"
install -Dm644 data/dev.so1ve.Relvi.svg "$HOME/.local/share/icons/hicolor/scalable/apps/dev.so1ve.Relvi.svg"
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

**Niri**:

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

**Sway**:

```text
exec relvi daemon
bindsym $mod+space exec relvi toggle
```

Run `relvi` to show the launcher directly, `relvi quit` to stop the resident process, or `relvi clear-history` to erase launch and query history. `Enter` or a single click opens the selected app. Use the arrow keys or `Ctrl+J/K` and `Ctrl+N/P` to change the selection. `Ctrl+D` scrolls down half a page; `Ctrl+U` scrolls up half a page.

Category tabs filter the current search. Scroll over the category bar to browse the tabs. Use `Ctrl+H` / `Ctrl+L` or `Shift+Tab` / `Tab` to select the previous / next category while keeping the search field focused. `Escape` hides the launcher.

## LICENSE

[MIT](LICENSE). Made with ❤️ by [Ray](https://github.com/so1ve)
