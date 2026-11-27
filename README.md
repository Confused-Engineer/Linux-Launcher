# Linux Launcher

A Linux-inspired, grid-style app launcher for Windows, in the spirit of the
COSMIC / GNOME app grids. Pin programs, shortcuts, folders, scripts, websites
and URIs, organise them into groups, and launch them with a click or a few
keystrokes.

## Features

- **Fast search:** start typing anywhere. Search matches names, groups, tags and
  paths, and **Enter** launches the best match.
- **Groups and favorites:** groups show as tabs (and as sections in *All*), and
  ★ Favorites gets its own tab.
- **Drag to organise:** drag tiles to reorder them, or drop one on a tab to move
  it into that group or into Favorites.
- **Easy to add apps:**
  - drag `.exe`/`.lnk` files, folders or documents from Explorer onto the window;
  - click **➕ Add app** (or press **Ctrl+N**) and use the file/folder pickers;
  - right-click → **Duplicate** to clone a similar entry.
- **Automatic icons:** the real Windows shell icon is used for programs,
  shortcuts and folders. You can override it with any image file, URL, or
  `.exe`/`.dll`. Entries with no icon get a coloured letter tile.
- **Run as administrator** from the right-click menu, with **Ctrl+Shift+Click**,
  or by default per app.
- `%ENV_VARS%` are expanded in targets, arguments and paths.
- **Human-friendly TOML config** that saves atomically and is never overwritten
  if it fails to parse. Export/import is included, plus automatic migration of
  the old `LL.ini`.
- Light/dark/system theme, adjustable icon size, and an option to minimize or
  close after launching.
- **Portable mode:** put an empty file named `portable` next to the `.exe`.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| *type* | Search |
| Enter | Launch the first result |
| Ctrl+Shift+Enter / Ctrl+Shift+Click | Launch as administrator |
| Esc | Clear search / leave settings / cancel dialog |
| Ctrl+N | Add an app |
| Ctrl+, | Settings |
| Ctrl+Enter | Save in the app editor |

## Configuration

| Mode | Location |
| --- | --- |
| Installed (default) | `%APPDATA%\LinuxLauncher\config.toml` |
| Portable | `config.toml` next to `LinuxLauncher.exe` (enabled by a `portable` marker file or an existing `config.toml` there) |

You can edit everything in the app, but the file is meant to be hand-editable
too (Settings → Data & backup → *Edit file*, then *Reload*):

```toml
version = 1

[settings]
icon_size = 80.0
theme = "system"          # system | dark | light
after_launch = "nothing"  # nothing | minimize | close
show_favorites = true
group_headers = true      # split the "All" tab into sections per group

[[groups]]
name = "Dev"

[[apps]]
name = "VS Code"
target = '%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe'
args = "--new-window"
group = "Dev"
tags = ["editor"]
favorite = true

[[apps]]
name = "Sound settings"
target = "ms-settings:sound"
```

Fields per app: `name`, `target` (required) and the optional `args`,
`working_dir`, `icon`, `group`, `tags`, `favorite` and `run_as_admin`. The
order of `[[apps]]` and `[[groups]]` is the display order. See
[`assets/default-config.toml`](assets/default-config.toml) for the annotated
starter config.

### Upgrading from 1.x

On first start, an existing `LL.ini` next to the executable (or in
`~/.linuxlauncher/`) is converted automatically, keeping its `weight` order.
The old file is left untouched. You can also import an `.ini` later from
Settings → Data & backup.

## Building

Requires a recent stable Rust toolchain (edition 2024, MSRV 1.95) on Windows.

```sh
cargo run                 # debug build
cargo test                # unit tests
cargo clippy --all-targets -- -D warnings
cargo build --release     # target/release/LinuxLauncher.exe
```

The installer is built with [Inno Setup](https://jrsoftware.org/isinfo.php)
from `assets/Setup.iss`; see `.github/workflows/release.yml`.

## Project layout

```
src/
  main.rs          window setup and startup
  cpu.rs           window loop + CPU renderer (winit, softbuffer, egui rasterizer)
  app.rs           LauncherApp: state, actions, dialogs
  session.rs       remembered tab and window placement (session.toml)
  config/          data model + group/ordering ops, TOML store, LL.ini migration
  search.rs        ranked multi-term search
  launch.rs        ShellExecuteExW launching, %VAR% expansion, reveal in Explorer
  icons.rs         icon resolution + background shell-icon extraction
  ui/              egui views: window chrome, launcher grid, tile, editor,
                   settings, toasts
```

The UI is drawn on the CPU (no Direct3D/OpenGL), like the other Confused
Engineer tools, so it looks and behaves the same on GPU-less VMs, over RDP and
on real hardware. Windows 11 rounds the window corners natively; on Windows 10
they are square.

The core modules (`config`, `search`, `launch`, `icons::resolve`) contain no UI
code and are unit tested. UI code reports user intent as `ui::Action` values,
which `app.rs` applies in one place.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
