<p align="center">
  <img src="data/dev.so1ve.Relvi.svg" width="96" height="96" alt="Relvi logo" />
</p>

<h1 align="center">Relvi</h1>
<h4 align="center">A focused application launcher for Wayland</h4>

Open Relvi with a shortcut, type what you remember, and press Enter to launch an app.

Relvi deliberately keeps its scope small: application search and launching. The goal is to make finding the app you want straightforward, with useful defaults and forgiving matching.

- **Forgiving search:** find apps by partial names, keywords, or executable names, even with typos.
- **Pinyin input:** find Chinese apps using their names, full Pinyin, or initials.
- **Relevant results:** exact matches take priority; history helps order similarly matched results. An empty search shows frequent and recent apps first.
- **Resident mode:** keep the app catalog in memory between uses, with automatic updates when installed apps change.
- **Keyboard navigation:** move through results and categories without leaving the search field.

Search is powered by [Polysearch](https://github.com/so1ve/polysearch), a practical mix of matching and ranking heuristics, not a single rigid algorithm. I refine its behavior based on real searches and everyday use, so matches and their order may change between releases. Examples of unexpected results help guide those changes.

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

Run `relvi` to show the launcher directly, `relvi quit` to stop the resident process, or `relvi clear-history` to erase launch and query history. `Enter` or a single click opens the selected app. Use the arrow keys or `Ctrl+J/K` and `Ctrl+N/P` to change the selection.

Category tabs filter the current search. Scroll over the category bar to browse the tabs. Use `Ctrl+H` / `Ctrl+L` or `Shift+Tab` / `Tab` to select the previous / next category while keeping the search field focused. `Escape` hides the launcher.

## LICENSE

[MIT](LICENSE). Made with ❤️ by [Ray](https://github.com/so1ve)
