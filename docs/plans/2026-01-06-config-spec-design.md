# Config File Specification

## Overview

Replace the current JSON config with TOML for better human readability. Add configuration for all previously hardcoded values.

**Config location:** `~/.local/share/kpick/config.toml`

## Complete Specification

```toml
# Path to KeePass database (supports ~ for home directory)
database_path = "~/passwords.kdbx"

# Seconds before clipboard is cleared (0 = never)
clipboard_timeout = 10

# Milliseconds to show the input flash indicator
flash_duration = 150

[window.password]
# Fixed dimensions in pixels
width = 400
height = 172
# Cap at this percentage of screen
max_percent = 40

[window.picker]
# Dimensions as percentage of screen
width_percent = 50
height_percent = 40
# Maximum entries visible in list
max_entries = 10

[font]
# Font family name (resolved from system fonts)
family = "DejaVu Sans"
# Font sizes in pixels
size = 18
hints_size = 14

[colors]
background = "#1e1e1e"
background_light = "#2d2d2d"
selection = "#264f78"
foreground = "#cccccc"
foreground_subtle = "#6e6e6e"
foreground_bright = "#ffffff"
error = "#ff6b6b"
```

## Defaults

All fields are optional. Missing fields use the defaults shown above.

## Implementation Notes

### Font Resolution

Search these directories for a font matching the family name:
- `/usr/share/fonts/`
- `~/.local/share/fonts/`
- `/run/current-system/sw/share/X11/fonts/` (NixOS)

Match by filename containing the family name (case-insensitive). Fall back to any available TTF if not found.

### Path Expansion

The `database_path` field supports:
- `~` expands to home directory
- Relative paths resolve from current working directory

### Migration

On first load, if `config.json` exists but `config.toml` does not:
1. Load the JSON config
2. Write equivalent TOML config
3. Keep JSON as backup (don't delete)

### Frecency Data

The `frecency` data (usage tracking) moves to a separate file `~/.local/share/kpick/frecency.json` since it's programmatic data, not user configuration.
