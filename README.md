<p align="center">
  <img width="128" height="128" alt="Kaede logo" src="https://github.com/user-attachments/assets/287d1e40-041e-4d86-ac22-dbccc70dbaa6" />
</p>

<h1 align="center">Kaede</h1>

<p align="center">
  <strong>A modern GPU management utility for Linux desktop environments.</strong><br />
  Explicitly control hardware acceleration by selecting specific GPUs for applications and games.
</p>

<p align="center">
  <a href="https://github.com/esther/KaedeGPU">
    <img src="https://img.shields.io/badge/Platform-Linux-1793D1?style=for-the-badge&logo=linux&logoColor=white" alt="Platform" />
  </a>
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/Language-Rust-黑色?style=for-the-badge&logo=rust&logoColor=white" alt="Language" />
  </a>
  <a href="https://gnome.pages.gitlab.gnome.org/libadwaita/">
    <img src="https://img.shields.io/badge/UI-Libadwaita-4A86CF?style=for-the-badge&logo=gnome&logoColor=white" alt="UI" />
  </a>
  <a href="LICENSE">
    <img src="https://img.shields.io/badge/License-GPL--3.0-blue?style=for-the-badge" alt="License" />
  </a>
</p>

<p align="center">
  <img width="1120" height="670" alt="Kaede screenshot" src="https://github.com/user-attachments/assets/659a1336-b443-4606-94b0-a5dd35b9192a" style="border-radius: 12px; box-shadow: 0 10px 30px rgba(0,0,0,0.3);" />
</p>

---

## Core Features

- **Hardware Discovery**: Automatically detects available GPUs via `/sys/class/drm`, `lspci`, and `/dev/dri/renderD*`.
- **Renderer Analysis**: Evaluates active renderers using `glxinfo -B` with Vulkan fallback support.
- **Application Scanning**: Indexes `.desktop` files across system and user-wide paths:
  - `/usr/share/applications`
  - `/usr/local/share/applications`
  - `~/.local/share/applications`
  - `/var/lib/flatpak/exports/share/applications`
  - `~/.local/share/flatpak/exports/share/applications`
- **User-Centric Management**: Searchable interface with per-app GPU preference selection.
- **Non-Destructive Overrides**: Modifies local user configurations without altering system-level files.

## Integration Targets

| Ecosystem | Implementation Strategy |
| :--- | :--- |
| **Native Binaries** | Generates localized `.desktop` overrides in `~/.local/share/applications`. |
| **Flatpak Apps** | Executes `flatpak override --user` to inject environment variables. |
| **Steam (Proton)** | Dynamically updates `LaunchOptions` in `localconfig.vdf` with automated backups. |
| **Heroic Launcher** | Modifies JSON configuration files within `GamesConfig` for precise environment control. |

## GPU Driver Orchestration

Kaede manages the following environment variables to ensure optimal hardware utilization:

- `DRI_PRIME` for Mesa/Open Source drivers.
- `PRESSURE_VESSEL_IMPORT_VARS` for Steam Runtime compatibility.
- **NVIDIA Prime Offload**:
  - `__NV_PRIME_RENDER_OFFLOAD=1`
  - `__GLX_VENDOR_LIBRARY_NAME=nvidia`
  - `__VK_LAYER_NV_optimus=NVIDIA_only`
- **Mesa Vulkan Layer**:
  - `MESA_VK_DEVICE_SELECT`
  - `MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE`

## Installation

### Prerequisites

Ensure your system has the following components:
- **GNOME / Libadwaita**: Runtime libraries for the GTK4 interface.
- **Development Tools**: Cargo and Rust toolchain (1.75+ recommended).
- **Optional Dependencies**: `pciutils` (lspci), `mesa-utils` (glxinfo), and `vulkan-tools` (vulkaninfo) for enhanced telemetry.

### Build from Source

```bash
git clone https://github.com/SterTheStar/kaede.git
cd kaede
cargo build --release
./target/release/kaede
```

## Configuration

Persistent settings are stored in TOML format:
`~/.config/kaede/config.toml`

## License

Distributed under the **GNU General Public License v3.0**. See `LICENSE` for more information.

---
<p align="center">
  Developed by <a href="https://github.com/SterTheStar">Esther</a>
</p>
