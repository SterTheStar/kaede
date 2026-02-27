<p align="center">
  <img width="128" height="128" alt="Kaede logo" src="https://github.com/user-attachments/assets/287d1e40-041e-4d86-ac22-dbccc70dbaa6" />
</p>

<h1 align="center">Kaede</h1>

<p align="center">
  <strong>A simple GPU manager for Linux.</strong><br />
  Choose which GPU an application or game should run on.
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Linux-1793D1?style=flat-square&logo=linux&logoColor=white" />
  <img alt="Language" src="https://img.shields.io/badge/language-Rust%201.75%2B-000000?style=flat-square&logo=rust" />
  <img alt="UI" src="https://img.shields.io/badge/UI-GTK4%20%2F%20libadwaita-4A86CF?style=flat-square&logo=gnome" />
  <img alt="License" src="https://img.shields.io/badge/license-GPL--3.0-blue.svg?style=flat-square" />
</p>

<p align="center">
  <img width="1120" height="670" alt="Kaede screenshot" src="https://github.com/user-attachments/assets/a84cc4cf-5b79-4c0f-9e3a-eb41ed7f466d" style="border-radius: 12px; box-shadow: 0 10px 30px rgba(0,0,0,0.3);" />
</p>

---

## Core Features

* Automatic GPU discovery using `/sys/class/drm`, `lspci`, and `/dev/dri/renderD*`.
* Renderer inspection via `glxinfo -B`, with Vulkan fallback support.
* Application indexing from standard `.desktop` locations:

```
/usr/share/applications
/usr/local/share/applications
~/.local/share/applications
/var/lib/flatpak/exports/share/applications
~/.local/share/flatpak/exports/share/applications
```

* Searchable interface with per-application GPU selection.
* User-level overrides that never modify system files.

## Integration

Kaede integrates with several Linux application ecosystems:

| Target              | Method                                                         |
| ------------------- | -------------------------------------------------------------- |
| Native applications | Creates `.desktop` overrides in `~/.local/share/applications`  |
| Flatpak             | Uses `flatpak override --user` to inject environment variables |
| Steam (Proton)      | Updates `LaunchOptions` in `localconfig.vdf`                   |
| Heroic Launcher     | Edits environment configuration inside `GamesConfig`           |

## GPU Environment Handling

Kaede configures environment variables used by common Linux GPU stacks.

**Mesa / Open drivers**

```
DRI_PRIME
MESA_VK_DEVICE_SELECT
MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE
```

**Steam runtime**

```
PRESSURE_VESSEL_IMPORT_VARS
```

**NVIDIA Prime Render Offload**

```
__NV_PRIME_RENDER_OFFLOAD=1
__GLX_VENDOR_LIBRARY_NAME=nvidia
__VK_LAYER_NV_optimus=NVIDIA_only
```

## Build from Source

```bash
git clone https://github.com/SterTheStar/kaede.git
cd kaede
cargo build --release
./target/release/kaede
```

## Configuration

Configuration is stored in:

```
~/.config/kaede/config.toml
```

## License

Released under the GNU General Public License v3.0.
See `LICENSE` for details.

---

<p align="center">
  Developed by <a href="https://github.com/SterTheStar">Esther</a>
</p>

---
<p align="center">
  Developed by <a href="https://github.com/SterTheStar">Esther</a>
</p>
