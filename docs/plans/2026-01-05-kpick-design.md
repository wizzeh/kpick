# kpick - wofi-style KeePassXC Password Launcher

## Overview

A wofi-style launcher for KeePassXC. Invoke it, type to fuzzy-search passwords, press Enter to copy the selected password to clipboard.

## Architecture

```
┌─────────────────────────────────────────────┐
│                   kpick                     │
├─────────────────────────────────────────────┤
│  UI Layer (egui + smithay-client-toolkit)   │
│  - Layer-shell surface (overlay)            │
│  - Text input + filtered list               │
├─────────────────────────────────────────────┤
│  Core Logic                                 │
│  - Fuzzy search + frecency ranking          │
│  - Clipboard management (auto-clear)        │
├─────────────────────────────────────────────┤
│  KeePassXC Client                           │
│  - Browser integration protocol             │
│  - Encrypted JSON over Unix socket          │
└─────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│  KeePassXC (running, database unlocked)     │
└─────────────────────────────────────────────┘
```

## KeePassXC Browser Integration Protocol

### Connection Flow

1. Connect to Unix socket at `$XDG_RUNTIME_DIR/kpxc_server`
2. Key exchange - generate keypair, exchange public keys with KeePassXC
3. Associate (first run) - KeePassXC prompts user to approve client
4. Get logins - fetch entries with title, username, password

### Messages

- `change-public-keys` - Initial handshake
- `associate` - One-time pairing
- `test-associate` - Verify stored association works
- `get-logins` - Fetch entries

### Error States

- Socket missing → "KeePassXC is not running"
- Database locked → "Please unlock your database"
- Not associated → Trigger association flow

## UI & Interaction

### Layout

```
┌────────────────────────────────────────┐
│  🔍 git                                │  ← Text input (focused on launch)
├────────────────────────────────────────┤
│  > GitHub - alice@example.com          │  ← Selected (highlighted)
│    GitLab - alice@work.org             │
│    DigitalOcean - git-deploy           │
└────────────────────────────────────────┘
```

### Keyboard

- **Type** → Filter list in real-time
- **↑/↓** → Move selection
- **Enter** → Copy password, close
- **Escape** → Close without action

### Behavior

- Opens as layer-shell overlay (above all windows, no decorations)
- Grabs keyboard focus immediately
- Closes on Escape, Enter, or click outside
- List shows top ~10 matches (scrollable if needed)
- Empty query → Show all entries sorted by frecency

### Search & Ranking

Fuzzy matching with smart ranking:
1. Exact matches
2. Prefix matches
3. Substring matches
4. Fuzzy matches

Frecency (frequency + recency) boosts entries used often and recently.

### Clipboard

- Copy password on Enter
- Auto-clear after 10 seconds

## Project Structure

```
kpick/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, CLI args
│   ├── ui.rs                # egui UI + smithay layer-shell
│   ├── keepassxc/
│   │   ├── mod.rs
│   │   ├── protocol.rs      # Message types, crypto
│   │   └── client.rs        # Socket connection, API calls
│   ├── search.rs            # Fuzzy matching + frecency ranking
│   └── clipboard.rs         # Copy + auto-clear timer
└── README.md
```

## Storage

XDG-compliant paths:
- Config: `~/.config/kpick/` (unused in MVP)
- Data: `~/.local/share/kpick/`
  - `association.json` - KeePassXC association ID + key
  - `frecency.json` - Entry usage stats

## Dependencies

- `smithay-client-toolkit` - Wayland layer-shell
- `egui` - Immediate-mode UI
- `nucleo` - Fuzzy matching
- `arboard` - Clipboard access
- `crypto_box` or `sodiumoxide` - Protocol encryption
- `serde`, `serde_json` - Serialization
- `directories` - XDG paths

## MVP Scope

### In Scope

- Connect to KeePassXC via browser protocol
- One-time association flow
- Fetch all entries, fuzzy search with frecency
- Display title + username
- Copy password to clipboard on selection
- Auto-clear clipboard after 10s
- Persist association + frecency to XDG paths
- Error messages for "not running" / "database locked"

### Out of Scope

- Config file (hardcode defaults)
- Theming / custom colors
- Auto-type
- TOTP support
- Launch KeePassXC automatically
- Copy username (only password)
