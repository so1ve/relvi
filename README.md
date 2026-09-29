<p align="center">
  <img src="data/dev.so1ve.Relvi.svg" width="96" height="96" alt="Relvi logo" />
</p>

<h1 align="center">Relvi</h1>
<h4 align="center">A small launcher and clipboard history for Wayland</h4>

Relvi focuses on finding apps and reusing copied text. Start it once, then open either view with a keyboard shortcut.

## Launcher

<p align="center">
  <img src="docs/screenshots/launcher.png" width="540" alt="Relvi launcher with a sample app catalog" />
</p>

### Search and history

Search app names, keywords, or executable names, including typos. Chinese names also match full Pinyin and initials. App names rank above descriptions; history helps order similar matches. With no query, frequent and recently used apps come first.

Category tabs filter the results. The catalog updates when apps are installed or removed.

Search uses [Polysearch](https://github.com/so1ve/polysearch). I tune its matching rules based on everyday use.

### Shortcuts

| Action | Shortcut |
| --- | --- |
| Open an app | `Enter` or a single click |
| Previous / next result | `↑` / `↓`, `Ctrl+K` / `Ctrl+J`, or `Ctrl+P` / `Ctrl+N` |
| Scroll half a page | `Ctrl+U` / `Ctrl+D` |
| Previous / next category | `Shift+Tab` / `Tab` or `Ctrl+H` / `Ctrl+L` |
| Close | `Escape` |

Show the launcher with `relvi`, or show/hide it with `relvi toggle`. Clear launch and query history with `relvi clear-history`.

## Clipboard

<p align="center">
  <img src="docs/screenshots/clipboard.png" width="720" alt="Relvi clipboard history with sample text and a preview" />
</p>

### Browse and copy

Search the full text of saved entries. Select a record on the left to preview it on the right. Copying closes the window; paste in the target app as usual.

Open this view with `relvi clipboard`, or show/hide it with `relvi clipboard toggle`. Toggling also switches from the launcher to clipboard history.

### Shortcuts

| Action | Shortcut |
| --- | --- |
| Previous / next entry | `↑` / `↓`, `Ctrl+K` / `Ctrl+J`, or `Ctrl+P` / `Ctrl+N` |
| Scroll half a page | `Ctrl+U` / `Ctrl+D` |
| Focus search | Start typing, or press `←` / `→` |
| Copy selected entry | `Ctrl+C`, `Enter`, or the Copy button |
| Delete selected entry | `Ctrl+Delete` |
| Clear history | `Ctrl+Shift+Delete` or the Clear button |
| Close | `Escape` |

Up/down navigation returns focus to the last selected entry.

### Storage

Relvi records text while it is running and keeps up to 100 entries of at most 64 KiB each. Duplicates move to the front. Entries marked by password managers are excluded.

History survives restarts in `$XDG_STATE_HOME/relvi/clipboard.json` (normally `~/.local/state/relvi/clipboard.json`). The file is plain text, with access restricted to your user. Run `relvi clipboard clear` to erase history without changing the current clipboard.

Clipboard monitoring requires `ext-data-control` or `wlr-data-control` support in the compositor.

## Install

### Nix

Run directly:

```sh
nix run github:so1ve/relvi
```

Or add the flake to your configuration:

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

### From source

Build with nightly Rust and the GTK 4, gtk4-layer-shell, libadwaita, and pkg-config development packages installed:

```sh
cargo build --locked --release
install -Dm755 target/release/relvi "$HOME/.local/bin/relvi"
install -Dm644 data/dev.so1ve.Relvi.desktop "$HOME/.local/share/applications/dev.so1ve.Relvi.desktop"
install -Dm644 data/dev.so1ve.Relvi.svg "$HOME/.local/share/icons/hicolor/scalable/apps/dev.so1ve.Relvi.svg"
```

Ensure `~/.local/bin` is on the graphical session's `PATH`. Released Linux binaries also need compatible GTK 4, gtk4-layer-shell, and libadwaita libraries at runtime.

## Session setup

### Startup and keybindings

Start `relvi daemon` with your Wayland session. This keeps the app catalog in memory and records clipboard history between uses. Bind `relvi toggle` and `relvi clipboard toggle` to separate shortcuts.

<details>
<summary>Niri</summary>

```kdl
spawn-at-startup "relvi" "daemon"

binds {
    Alt+Space { spawn "relvi" "toggle"; }
    Alt+V { spawn "relvi" "clipboard" "toggle"; }
}
```

</details>

<details>
<summary>Hyprland</summary>

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("relvi daemon")
end)

hl.bind("SUPER + SPACE", hl.dsp.exec_cmd("relvi toggle"))
hl.bind("SUPER + V", hl.dsp.exec_cmd("relvi clipboard toggle"))
```

</details>

<details>
<summary>Sway</summary>

```text
exec relvi daemon
bindsym $mod+space exec relvi toggle
bindsym $mod+v exec relvi clipboard toggle
```

</details>

Run `relvi quit` to stop the resident process.

### Shell completions

Generate completions for your shell, for example:

```sh
relvi completions --shell fish
```

Supported shells: Bash, Elvish, Fish, PowerShell, and Zsh.

## License

[MIT](LICENSE). Made with ❤️ by [Ray](https://github.com/so1ve)
