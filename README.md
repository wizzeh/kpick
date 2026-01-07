# kpick

A fast, fuzzy password picker for KeePass databases on Wayland.

## Features

- **Fast fuzzy search** - Find entries quickly with nucleo-matcher
- **Frecency ranking** - Recently and frequently used entries appear first
- **Secure** - Passwords are zeroed from memory on drop
- **Auto-clear clipboard** - Passwords are cleared after a configurable timeout
- **Native Wayland** - Layer-shell popup, no X11 dependencies
- **Configurable** - Colors, fonts, window sizes, and more

## Requirements

- Wayland compositor with layer-shell support
- `wl-clipboard` (`wl-copy` and `wl-paste` commands)
- A TTF font (e.g., DejaVu Sans, Liberation Sans)

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Use database from config file
kpick

# Override database path
kpick --database ~/passwords.kdbx
kpick -d ~/passwords.kdbx
```

### Keyboard Shortcuts

**Password prompt:**
- `Enter` - Unlock database
- `Escape` - Cancel

**Entry picker:**
- `Up/Down` - Navigate entries
- `Enter` - Copy password
- `Shift+Enter` - Copy username
- `Escape` - Cancel

## Configuration

Configuration is stored at `~/.local/share/kpick/config.toml`. Create it manually or run kpick once to see the default location.

### Example Configuration

```toml
# Path to KeePass database (supports ~ expansion)
database_path = "~/passwords.kdbx"

# Seconds before clipboard is cleared (0 = never)
clipboard_timeout = 10

# Milliseconds to show input flash indicator
flash_duration = 150

[window.password]
width = 400
height = 172
max_percent = 40

[window.picker]
width_percent = 50
height_percent = 40
max_entries = 10

[font]
family = "DejaVu Sans"
size = 18.0
hints_size = 14.0

[colors]
background = "#1e1e1e"
background_light = "#2d2d2d"
selection = "#264f78"
foreground = "#cccccc"
foreground_subtle = "#6e6e6e"
foreground_bright = "#ffffff"
error = "#ff6b6b"
```

## Building from Source

```bash
git clone https://github.com/your-username/kpick
cd kpick
cargo build --release
```

## License

GPL-3.0 - see [LICENSE](LICENSE) for details.
