# Kaede

Kaede is a Linux desktop app (Rust + GTK4/libadwaita) to assign applications and games to a specific GPU with a visual interface.

## Highlights

- Detects GPUs using `/sys/class/drm`, `lspci`, and `/dev/dri/renderD*`
- Detects renderer info using `glxinfo -B` (with Vulkan fallback)
- Scans apps from `.desktop` files in:
  - `/usr/share/applications`
  - `/usr/local/share/applications`
  - `~/.local/share/applications`
  - `/var/lib/flatpak/exports/share/applications`
  - `~/.local/share/flatpak/exports/share/applications`
- Searchable app list with per-app GPU selector
- Safe user-level overrides (does not overwrite system launchers)

## Supported Targets

- Native `.desktop` apps
  - Generates user overrides in `~/.local/share/applications`
- Flatpak apps
  - Uses `flatpak override --user` env vars
- Steam games
  - Edits per-game `LaunchOptions` in `localconfig.vdf`
  - Creates backup automatically (`localconfig.vdf.kaede.bak`)
- Heroic games
  - Edits per-game `GamesConfig/*.json` env options
  - Creates backup automatically (`*.json.kaede.bak`)

## GPU Variables Used

Depending on selected GPU/driver stack, Kaede can apply:

- `DRI_PRIME`
- `PRESSURE_VESSEL_IMPORT_VARS` (Steam/Proton)
- NVIDIA offload vars:
  - `__NV_PRIME_RENDER_OFFLOAD=1`
  - `__GLX_VENDOR_LIBRARY_NAME=nvidia`
  - `__VK_LAYER_NV_optimus=NVIDIA_only`
- Mesa Vulkan selection vars:
  - `MESA_VK_DEVICE_SELECT`
  - `MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE`

## Config

Kaede stores per-app GPU preferences in:

- `~/.config/kaede/config.toml`

## Build and Run

```bash
cargo build
cargo run
```

## Requirements

- Linux with GTK4 and libadwaita runtime/development packages
- Optional tools for richer detection: `lspci`, `glxinfo`, `vulkaninfo`
- `flatpak` command available for Flatpak overrides

## License

This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)**.
