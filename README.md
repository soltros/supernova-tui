# supernova-tui

A terminal user interface (TUI) client for the [Supernova](https://github.com/soltros/supernova) music server, built in Rust with [`ratatui`](https://ratatui.rs).

`supernova-tui` is strictly a client—it connects to an existing Supernova server over HTTP(S), plays audio locally via `mpv`, and supports every core Supernova feature and server plugin.

## Features

- **Dashboard & Discovery (`1:Home`)**: Recently added albums, recently played tracks, favorite tracks, Release Radar, and Similar Artists.
- **Library Browser (`2:Library`)**: 3-column Artists → Albums → Tracks browser, artist letter filter, queue management, hearting, playlist assignment, and track downloads.
- **Unified Search (`3:Search`)**: Search across artists, albums, and tracks.
- **Playlists (`4:Playlists`)**: Create, delete, browse, play, remove tracks, and export backup JSON.
- **Hearts (`5:Hearts`)**: Manage favorited tracks, albums, artists, and playlists, plus backup export.
- **All Supernova Plugins Supported**:
  - **PodcastIndex (`6:Podcasts`)**: Search podcasts, subscribe/unsubscribe, browse episodes, stream with auto-saved resume position, mark completed, and export/import OPML.
  - **RadioBrowser (`7:Radio`)**: Search internet radio stations by name or `country:<CODE>`, subscribe/unsubscribe, heart stations, and stream live.
  - **LRCLib (`8:Lyrics`)**: Live time-synced LRC lyrics highlighted in sync with `mpv` playback (with plain-lyrics fallback).
  - **Last.fm (`9:Plugins` / `0:Settings`)**: In-app auth URL generation, token exchange, automatic Now Playing updates, and automatic scrobbling.
  - **Subsonic, Auto-Tagger, ArtistMerger, AlbumMerger, Deduper (`9:Plugins`)**: View plugin status, test `/rest/ping`, trigger admin jobs, and inspect read-only merge/dedupe previews.
- **In-App Settings (`0:Settings`)**: Edit server URL, switch account, change password, configure stream transcode format/bitrate, adjust default volume, manage Last.fm session, and check server health. All settings persist to `~/.config/supernova/config.json`.
- **Smart Self-Update (`supernova-tui update`)**: Detects OS/architecture (`x86_64`/`aarch64`) and installation method (`.deb`, `.rpm`, AUR, Nix, or standalone binary) and downloads/installs the matching GitHub release asset.

## Requirements

- `mpv` in `$PATH` for audio playback.

## Configuration

All client settings are stored in `~/.config/supernova/config.json` (with automatic fallback to `~/.config/supernova/credentials.json` if you previously logged in with `sn`).

```json
{
  "url": "http://localhost:8080",
  "token": "...",
  "lastfm_session_key": null,
  "stream_format": "mp3",
  "stream_bitrate": 320,
  "volume": 100
}
```

## Installation

### Debian / Ubuntu (`.deb`)
```bash
sudo dpkg -i supernova-tui_<version>_amd64.deb
```

### Fedora / RHEL / openSUSE (`.rpm`)
```bash
sudo rpm -Uvh supernova-tui-<version>-1.x86_64.rpm
```

### Arch Linux (AUR `PKGBUILD`)
```bash
cd packaging/aur
makepkg -si
```

### Nix (Flake)
```bash
nix run github:soltros/supernova-tui
# or install to profile:
nix profile install github:soltros/supernova-tui
```

### Generic Linux Binary
Download `supernova-tui-linux-x86_64.tar.gz` (or `aarch64`) from [Releases](https://github.com/soltros/supernova-tui/releases):
```bash
tar -xzf supernova-tui-linux-x86_64.tar.gz
sudo install -m 755 supernova-tui /usr/local/bin/supernova-tui
```

### From Source
```bash
cargo build --release
```

## Usage

```bash
supernova-tui --url http://localhost:8080
```

### Self-Update
```bash
supernova-tui update --check   # Check latest release & detected package type
supernova-tui update           # Download and install matching release asset
```

## Keybindings

| Key | Action |
| --- | --- |
| `1`–`9`, `0` | Switch tabs (`0` = Settings) |
| `Space` | Play / Pause |
| `>` / `<` | Next / Previous track in queue |
| `[` / `]` | Seek -10s / +10s |
| `+` / `-` | Volume up / down |
| `u` | Toggle Queue modal |
| `f` | Toggle Heart |
| `q` | Add to queue |
| `a` | Add track to playlist |
| `/` | Search (Library, Podcasts, Radio) |
| `?` | Help modal |
| `Q` / `Ctrl+C` | Quit |
