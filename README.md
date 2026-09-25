# Relvi

Relvi is a small Wayland application launcher. It keeps an application catalog in memory, searches names, keywords and desktop IDs with [polysearch](https://github.com/so1ve/polysearch), and learns which apps you choose for a query.

## Install

Build with nightly Rust and the GTK 4, gtk4-layer-shell, and pkg-config development packages installed:

```sh
cargo build --locked --release
install -Dm755 target/release/relvi "$HOME/.local/bin/relvi"
install -Dm644 data/dev.so1ve.Relvi.desktop "$HOME/.local/share/applications/dev.so1ve.Relvi.desktop"
```

Ensure `~/.local/bin` is on the graphical session's `PATH`. The released Linux binaries also need GTK 4 and gtk4-layer-shell at runtime; if those system libraries are unavailable or incompatible, build locally.

## Use

Run `relvi daemon` once after the Wayland session starts. It loads the catalog without showing a window and stays resident. Then bind `relvi toggle` to a compositor shortcut. Repeated toggles use the same process and in-memory catalog; installed applications are refreshed when GIO reports changes.

For example, add one of the following to your compositor configuration (adjust `Mod`/`SUPER` to your preferred shortcut):

**Niri** (`config.kdl`):

```kdl
spawn-at-startup "relvi" "daemon"

binds {
    Mod+Space { spawn "relvi" "toggle"; }
}
```

**Hyprland** (current Lua configuration):

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
