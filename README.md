# supernova-tui

A terminal UI client for the [Supernova](https://github.com/soltros/supernova) music server, written in Rust with [ratatui](https://ratatui.rs).

`supernova-tui` is a client only. It connects to a Supernova server over HTTP/HTTPS and plays audio locally with `mpv`.

## Features

- **Dashboard & Discovery (`1:Home`)**: Recently added albums, recently played tracks, favorite tracks, Release Radar, and similar artists.
- **Library Browser (`2:Library`)**: 3-column Artists / Albums / Tracks browser with letter filtering, queuing, hearting, playlist assignment, and track downloads.
- **Search (`3:Search`)**: Search across artists, albums, and tracks.
- **Playlists (`4:Playlists`)**: Create, delete, browse, play, remove tracks, and export backup JSON.
- **Hearts (`5:Hearts`)**: Manage favorited tracks, albums, artists, and playlists, plus backup export.
- **Plugin Support**:
  - **PodcastIndex (`6:Podcasts`)**: Search podcasts, subscribe/unsubscribe, browse episodes, stream with saved resume position, mark episodes completed, and export/import OPML.
  - **RadioBrowser (`7:Radio`)**: Search internet radio stations by name or `country:<CODE>`, subscribe/unsubscribe, heart stations, and stream live.
  - **LRCLib (`8:Lyrics`)**: Time-synced LRC lyrics highlighted against `mpv` playback position (falls back to plain lyrics).
  - **Last.fm (`9:Plugins` / `0:Settings`)**: Auth URL generation, token exchange, Now Playing updates, and automatic scrobbling.
  - **Subsonic, Auto-Tagger, ArtistMerger, AlbumMerger, Deduper (`9:Plugins`)**: Plugin status list, `/rest/ping` check, admin job triggers, and read-only merge/dedupe previews.
- **Settings (`0:Settings`)**: Configure server URL, account login, password changes, transcode format/bitrate, default volume, Last.fm session, and server health checks. All settings are saved to `~/.config/supernova/config.json`.
- **Self-Update (`supernova-tui update`)**: Detects your OS, CPU architecture (`x86_64`/`aarch64`), and install type (`.deb`, `.rpm`, AUR, Nix, or standalone binary) and pulls the matching release asset from GitHub.

## Requirements

- `mpv` in `$PATH` for audio playback.

## Configuration

Client settings are stored in `~/.config/supernova/config.json`. If `config.json` does not exist yet or has no `url`/`token`, `supernova-tui` falls back to `~/.config/supernova/credentials.json` from the `sn` CLI.

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
supernova-tui update --check   # Check latest release and detected package type
supernova-tui update           # Download and install matching release asset
```

## Keybindings

| Key | Action |
| --- | --- |
| `1`-`9`, `0` | Switch tabs (`0` = Settings) |
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
