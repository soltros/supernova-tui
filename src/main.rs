mod app;
mod client;
mod config;
mod models;
mod player;
mod ui;
mod updater;

use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::{App, InputAction, Modal, PreviewKind, Tab};
use crate::client::SupernovaClient;
use crate::config::{load_config, save_config};

#[derive(Parser, Debug)]
#[command(name = "supernova-tui", version, about = "Ratatui TUI client for Supernova")]
struct Args {
    /// Supernova server URL (defaults to config.json or http://localhost:8080)
    #[arg(long)]
    url: Option<String>,

    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    /// Check for or install the latest release matching your OS/arch/package format
    Update {
        /// Only check if an update is available without installing
        #[arg(long)]
        check: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(CliCommand::Update { check }) = args.command {
        return updater::run_self_update(check).await;
    }

    let cfg = load_config();
    let base_url = args
        .url
        .or_else(|| cfg.url.clone())
        .unwrap_or_else(|| "http://localhost:8080".to_string());
    let token = cfg.token.clone();

    let mut app = App::new(base_url, token, cfg)?;
    if app.client.token.is_some() {
        app.init_authenticated().await;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, &mut app).await;

    app.player.shutdown();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

async fn run_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    let mut reader = event::EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(200));

    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        if app.should_quit {
            break;
        }

        tokio::select! {
            _ = ticker.tick() => {
                app.tick().await;
            }
            maybe_ev = reader.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_ev {
                    handle_key(app, key).await;
                }
            }
        }
    }
    Ok(())
}

async fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Handle active modal first
    match &app.modal {
        Modal::None => {}
        Modal::Help => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
                app.modal = Modal::None;
            }
            return;
        }
        Modal::Queue => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('u') => app.modal = Modal::None,
                KeyCode::Char('c') => {
                    app.queue.clear();
                    app.queue_idx = None;
                    app.status_msg = "Queue cleared".to_string();
                }
                _ => {}
            }
            return;
        }
        Modal::Login { .. } => {
            handle_login_key(app, key).await;
            return;
        }
        Modal::TextInput { .. } => {
            handle_input_key(app, key).await;
            return;
        }
        Modal::AddToPlaylist { .. } => {
            handle_add_playlist_key(app, key).await;
            return;
        }
        Modal::ChangePassword { .. } => {
            handle_change_password_key(app, key).await;
            return;
        }
    }

    // Global shortcuts
    match key.code {
        KeyCode::Char('Q') => {
            app.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            app.modal = Modal::Help;
            return;
        }
        KeyCode::Char('u') => {
            app.modal = Modal::Queue;
            return;
        }
        KeyCode::Char('0') => {
            app.tab = Tab::Settings;
            return;
        }
        KeyCode::Char('C') => {
            app.logout_and_prompt().await;
            return;
        }
        KeyCode::Char(' ') => {
            if let Ok(paused) = app.player.toggle_pause().await {
                app.playback.paused = paused;
            }
            return;
        }
        KeyCode::Char('>') => {
            app.play_next().await;
            return;
        }
        KeyCode::Char('<') => {
            app.play_prev().await;
            return;
        }
        KeyCode::Char(']') => {
            let _ = app.player.seek_rel(10.0).await;
            return;
        }
        KeyCode::Char('[') => {
            let _ = app.player.seek_rel(-10.0).await;
            return;
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.tui_config.volume = (app.tui_config.volume + 5).min(100);
            let _ = app.player.set_volume(app.tui_config.volume).await;
            app.save_cfg();
            return;
        }
        KeyCode::Char('-') => {
            app.tui_config.volume = app.tui_config.volume.saturating_sub(5);
            let _ = app.player.set_volume(app.tui_config.volume).await;
            app.save_cfg();
            return;
        }
        KeyCode::Char(c @ '1'..='9') => {
            let idx = (c as usize) - ('1' as usize);
            app.tab = Tab::ALL[idx];
            return;
        }
        _ => {}
    }

    match app.tab {
        Tab::Dashboard => handle_dashboard_key(app, key).await,
        Tab::Library => handle_library_key(app, key).await,
        Tab::Search => handle_search_key(app, key).await,
        Tab::Playlists => handle_playlists_key(app, key).await,
        Tab::Hearts => handle_hearts_key(app, key).await,
        Tab::Podcasts => handle_podcasts_key(app, key).await,
        Tab::Radio => handle_radio_key(app, key).await,
        Tab::Lyrics => handle_lyrics_key(app, key).await,
        Tab::Plugins => handle_plugins_key(app, key).await,
        Tab::Settings => handle_settings_key(app, key).await,
    }
}

async fn handle_login_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('r') {
        if let Modal::Login { is_register, field, .. } = &mut app.modal {
            *is_register = !*is_register;
            if !*is_register && *field == 3 {
                *field = 2;
            }
        }
        return;
    }

    match key.code {
        KeyCode::Tab | KeyCode::Down => {
            if let Modal::Login { field, is_register, .. } = &mut app.modal {
                let max = if *is_register { 4 } else { 3 };
                *field = (*field + 1) % max;
            }
        }
        KeyCode::BackTab | KeyCode::Up => {
            if let Modal::Login { field, is_register, .. } = &mut app.modal {
                let max = if *is_register { 4 } else { 3 };
                *field = (*field + max - 1) % max;
            }
        }
        KeyCode::Backspace => {
            if let Modal::Login { url, username, password, invite_code, field, .. } = &mut app.modal {
                match *field {
                    0 => { url.pop(); }
                    1 => { username.pop(); }
                    2 => { password.pop(); }
                    _ => { invite_code.pop(); }
                }
            }
        }
        KeyCode::Char(c) => {
            if let Modal::Login { url, username, password, invite_code, field, .. } = &mut app.modal {
                match *field {
                    0 => url.push(c),
                    1 => username.push(c),
                    2 => password.push(c),
                    _ => invite_code.push(c),
                }
            }
        }
        KeyCode::Enter => {
            app.submit_login().await;
        }
        KeyCode::Esc => {
            if app.user.is_some() {
                app.modal = Modal::None;
            }
        }
        _ => {}
    }
}

async fn handle_input_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.modal = Modal::None,
        KeyCode::Backspace => {
            if let Modal::TextInput { value, .. } = &mut app.modal {
                value.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Modal::TextInput { value, .. } = &mut app.modal {
                value.push(c);
            }
        }
        KeyCode::Enter => {
            let Modal::TextInput { value, action, .. } = std::mem::replace(&mut app.modal, Modal::None) else {
                return;
            };
            let val = value.trim().to_string();
            match action {
                InputAction::SearchLibrary => {
                    app.search_query = val.clone();
                    match app.client.search(&val).await {
                        Ok(r) => {
                            app.search_results = r;
                            app.search_sel = [0; 3];
                        }
                        Err(e) => app.status_msg = format!("Search failed: {e}"),
                    }
                }
                InputAction::CreatePlaylist => {
                    if !val.is_empty() {
                        match app.client.create_playlist(&val).await {
                            Ok(p) => {
                                app.status_msg = format!("Created playlist '{}'", p.name);
                                if let Ok(pls) = app.client.playlists().await {
                                    app.playlists = pls;
                                }
                            }
                            Err(e) => app.status_msg = format!("Create failed: {e}"),
                        }
                    }
                }
                InputAction::SearchPodcasts => {
                    if !val.is_empty() {
                        match app.client.podcast_search(&val).await {
                            Ok(feeds) => {
                                app.podcast_search = feeds;
                                app.podcast_showing_search = true;
                                app.podcast_feed_sel = 0;
                                app.podcast_pane = 0;
                                app.status_msg = format!("Found {} podcast feeds", app.podcast_search.len());
                            }
                            Err(e) => app.status_msg = format!("Podcast search failed: {e}"),
                        }
                    }
                }
                InputAction::ImportOpmlPath => {
                    if !val.is_empty() {
                        match tokio::fs::read(&val).await {
                            Ok(bytes) => {
                                let fname = std::path::Path::new(&val)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("import.opml");
                                match app.client.podcast_import_opml(fname, bytes).await {
                                    Ok(_) => {
                                        app.status_msg = "OPML import started".to_string();
                                        if let Ok(subs) = app.client.podcast_subscriptions().await {
                                            app.podcast_subs = subs;
                                        }
                                    }
                                    Err(e) => app.status_msg = format!("OPML import failed: {e}"),
                                }
                            }
                            Err(e) => app.status_msg = format!("Read file failed: {e}"),
                        }
                    }
                }
                InputAction::SearchRadio => {
                    if !val.is_empty() {
                        let (q, country) = if let Some(c) = val.strip_prefix("country:") {
                            ("", c.trim())
                        } else {
                            (val.as_str(), "")
                        };
                        match app.client.radio_search(q, country, 50, 0).await {
                            Ok(stations) => {
                                app.radio_search = stations;
                                app.radio_search_sel = 0;
                                app.radio_pane = 1;
                                app.status_msg = format!("Found {} radio stations", app.radio_search.len());
                            }
                            Err(e) => app.status_msg = format!("Radio search failed: {e}"),
                        }
                    }
                }
                InputAction::LastFmToken => {
                    if !val.is_empty() {
                        match app.client.lastfm_exchange_token(&val).await {
                            Ok(sk) => {
                                app.tui_config.lastfm_session_key = Some(sk);
                                app.save_cfg();
                                app.lastfm_auth_url = None;
                                app.status_msg = "Last.fm connected!".to_string();
                            }
                            Err(e) => app.status_msg = format!("Last.fm token error: {e}"),
                        }
                    }
                }
                InputAction::ArtistLetter => {
                    let letter = if val.is_empty() { None } else { Some(val.clone()) };
                    app.artist_letter = letter.clone();
                    if let Ok(arts) = app.client.artists(200, 0, letter.as_deref()).await {
                        app.artists = arts;
                        app.artist_sel = 0;
                    }
                }
                InputAction::EditServerUrl => {
                    let new_url = val.trim_end_matches('/').to_string();
                    if !new_url.is_empty() {
                        match SupernovaClient::new(new_url.clone(), app.tui_config.token.clone()) {
                            Ok(c) => {
                                app.client = c;
                                app.tui_config.url = Some(new_url.clone());
                                app.save_cfg();
                                app.status_msg = format!("Server URL updated to {new_url}");
                                app.init_authenticated().await;
                            }
                            Err(e) => app.status_msg = format!("Invalid URL: {e}"),
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

async fn handle_add_playlist_key(app: &mut App, key: KeyEvent) {
    let Modal::AddToPlaylist { track, selected } = &mut app.modal else {
        return;
    };
    let len = app.playlists.len();
    match key.code {
        KeyCode::Esc => app.modal = Modal::None,
        KeyCode::Up | KeyCode::Char('k') => *selected = selected.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected + 1 < len {
                *selected += 1;
            }
        }
        KeyCode::Enter => {
            if let Some(pl) = app.playlists.get(*selected).cloned() {
                let tid = track.id.clone();
                app.modal = Modal::None;
                match app.client.add_track_to_playlist(&pl.id, &tid).await {
                    Ok(()) => app.status_msg = format!("Added track to '{}'", pl.name),
                    Err(e) => app.status_msg = format!("Failed: {e}"),
                }
            }
        }
        _ => {}
    }
}

fn move_idx(idx: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        *idx = 0;
        return;
    }
    let next = (*idx as isize + delta).clamp(0, (len - 1) as isize);
    *idx = next as usize;
}

async fn handle_dashboard_key(app: &mut App, key: KeyEvent) {
    let lens = [
        app.dashboard.recently_added_albums.len(),
        app.dashboard.recently_played_tracks.len(),
        app.dashboard.favorite_tracks.len(),
        app.discovery.release_radar.len(),
        app.discovery.similar_artists.len(),
    ];
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.dash_pane = (app.dash_pane + 4) % 5,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.dash_pane = (app.dash_pane + 1) % 5,
        KeyCode::Up | KeyCode::Char('k') => move_idx(&mut app.dash_sel[app.dash_pane], lens[app.dash_pane], -1),
        KeyCode::Down | KeyCode::Char('j') => move_idx(&mut app.dash_sel[app.dash_pane], lens[app.dash_pane], 1),
        KeyCode::Char('r') => app.refresh_initial().await,
        KeyCode::Enter => match app.dash_pane {
            0 => {
                if let Some(alb) = app.dashboard.recently_added_albums.get(app.dash_sel[0]).cloned() {
                    app.albums = vec![alb.clone()];
                    app.album_sel = 0;
                    app.load_album_tracks(&alb.id).await;
                    app.lib_pane = 2;
                    app.tab = Tab::Library;
                }
            }
            1 => {
                let tracks = app.dashboard.recently_played_tracks.clone();
                app.play_tracks_from(tracks, app.dash_sel[1]).await;
            }
            2 => {
                let tracks = app.dashboard.favorite_tracks.clone();
                app.play_tracks_from(tracks, app.dash_sel[2]).await;
            }
            4 => {
                if let Some(sim) = app.discovery.similar_artists.get(app.dash_sel[4]).cloned() {
                    if !sim.id.is_empty() {
                        app.load_artist_albums(&sim.id).await;
                        app.lib_pane = 1;
                        app.tab = Tab::Library;
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
}

async fn handle_library_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.lib_pane = (app.lib_pane + 2) % 3,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.lib_pane = (app.lib_pane + 1) % 3,
        KeyCode::Up | KeyCode::Char('k') => match app.lib_pane {
            0 => {
                move_idx(&mut app.artist_sel, app.artists.len(), -1);
                if let Some(a) = app.artists.get(app.artist_sel).cloned() {
                    app.load_artist_albums(&a.id).await;
                }
            }
            1 => {
                move_idx(&mut app.album_sel, app.albums.len(), -1);
                if let Some(a) = app.albums.get(app.album_sel).cloned() {
                    app.load_album_tracks(&a.id).await;
                }
            }
            _ => move_idx(&mut app.lib_track_sel, app.lib_tracks.len(), -1),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.lib_pane {
            0 => {
                move_idx(&mut app.artist_sel, app.artists.len(), 1);
                if let Some(a) = app.artists.get(app.artist_sel).cloned() {
                    app.load_artist_albums(&a.id).await;
                }
            }
            1 => {
                move_idx(&mut app.album_sel, app.albums.len(), 1);
                if let Some(a) = app.albums.get(app.album_sel).cloned() {
                    app.load_album_tracks(&a.id).await;
                }
            }
            _ => move_idx(&mut app.lib_track_sel, app.lib_tracks.len(), 1),
        },
        KeyCode::Enter => match app.lib_pane {
            0 => {
                if let Some(a) = app.artists.get(app.artist_sel).cloned() {
                    app.load_artist_albums(&a.id).await;
                    app.lib_pane = 1;
                }
            }
            1 => {
                if let Some(a) = app.albums.get(app.album_sel).cloned() {
                    app.load_album_tracks(&a.id).await;
                    app.lib_pane = 2;
                }
            }
            _ => {
                let tracks = app.lib_tracks.clone();
                app.play_tracks_from(tracks, app.lib_track_sel).await;
            }
        },
        KeyCode::Char('q') => {
            if app.lib_pane == 2 {
                if let Some(t) = app.lib_tracks.get(app.lib_track_sel).cloned() {
                    app.queue.push(t.clone());
                    app.status_msg = format!("Queued '{}'", t.title);
                }
            } else if app.lib_pane == 1 {
                let added = app.lib_tracks.len();
                app.queue.extend(app.lib_tracks.clone());
                app.status_msg = format!("Queued {added} album tracks");
            }
        }
        KeyCode::Char('f') => match app.lib_pane {
            0 => {
                if let Some(a) = app.artists.get(app.artist_sel).cloned() {
                    app.toggle_heart("artist", &a.id, None).await;
                }
            }
            1 => {
                if let Some(a) = app.albums.get(app.album_sel).cloned() {
                    app.toggle_heart("album", &a.id, None).await;
                }
            }
            _ => {
                if let Some(t) = app.lib_tracks.get(app.lib_track_sel).cloned() {
                    app.toggle_heart("track", &t.id, None).await;
                }
            }
        },
        KeyCode::Char('a') => {
            if let Some(t) = app.lib_tracks.get(app.lib_track_sel).cloned() {
                app.modal = Modal::AddToPlaylist { track: t, selected: 0 };
            }
        }
        KeyCode::Char('A') => {
            if let Ok(albs) = app.client.albums(200, 0, None).await {
                app.albums = albs;
                app.album_sel = 0;
                app.lib_pane = 1;
                if let Some(first) = app.albums.first().cloned() {
                    app.load_album_tracks(&first.id).await;
                }
            }
        }
        KeyCode::Char('L') => {
            app.modal = Modal::TextInput {
                title: " Filter Artists by Letter ".to_string(),
                prompt: "Enter starting letter (empty for all):".to_string(),
                value: app.artist_letter.clone().unwrap_or_default(),
                action: InputAction::ArtistLetter,
            };
        }
        KeyCode::Char('D') => {
            if let Some(t) = app.lib_tracks.get(app.lib_track_sel).cloned() {
                let dl_dir = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                let safe_name = format!(
                    "{} - {}.{}",
                    t.artist_name.replace('/', "_"),
                    t.title.replace('/', "_"),
                    if t.format.is_empty() { "audio" } else { &t.format }
                );
                let dest = dl_dir.join(safe_name);
                match app.client.download_track_to(&t.id, &dest).await {
                    Ok(()) => app.status_msg = format!("Downloaded to {}", dest.display()),
                    Err(e) => app.status_msg = format!("Download failed: {e}"),
                }
            }
        }
        _ => {}
    }
}

async fn handle_search_key(app: &mut App, key: KeyEvent) {
    let lens = [
        app.search_results.artists.len(),
        app.search_results.albums.len(),
        app.search_results.tracks.len(),
    ];
    match key.code {
        KeyCode::Char('/') => {
            app.modal = Modal::TextInput {
                title: " Search Library ".to_string(),
                prompt: "Enter search query:".to_string(),
                value: app.search_query.clone(),
                action: InputAction::SearchLibrary,
            };
        }
        KeyCode::Left | KeyCode::Char('h') => app.search_pane = (app.search_pane + 2) % 3,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.search_pane = (app.search_pane + 1) % 3,
        KeyCode::Up | KeyCode::Char('k') => move_idx(&mut app.search_sel[app.search_pane], lens[app.search_pane], -1),
        KeyCode::Down | KeyCode::Char('j') => move_idx(&mut app.search_sel[app.search_pane], lens[app.search_pane], 1),
        KeyCode::Enter => match app.search_pane {
            0 => {
                if let Some(a) = app.search_results.artists.get(app.search_sel[0]).cloned() {
                    app.load_artist_albums(&a.id).await;
                    app.lib_pane = 1;
                    app.tab = Tab::Library;
                }
            }
            1 => {
                if let Some(a) = app.search_results.albums.get(app.search_sel[1]).cloned() {
                    app.albums = vec![a.clone()];
                    app.album_sel = 0;
                    app.load_album_tracks(&a.id).await;
                    app.lib_pane = 2;
                    app.tab = Tab::Library;
                }
            }
            _ => {
                let tracks = app.search_results.tracks.clone();
                app.play_tracks_from(tracks, app.search_sel[2]).await;
            }
        },
        _ => {}
    }
}

async fn handle_playlists_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.playlist_pane = 0,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.playlist_pane = 1,
        KeyCode::Up | KeyCode::Char('k') => {
            if app.playlist_pane == 0 {
                move_idx(&mut app.playlist_sel, app.playlists.len(), -1);
                if let Some(pl) = app.playlists.get(app.playlist_sel).cloned() {
                    if let Ok(ts) = app.client.playlist_tracks(&pl.id).await {
                        app.playlist_tracks = ts;
                        app.playlist_track_sel = 0;
                    }
                }
            } else {
                move_idx(&mut app.playlist_track_sel, app.playlist_tracks.len(), -1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.playlist_pane == 0 {
                move_idx(&mut app.playlist_sel, app.playlists.len(), 1);
                if let Some(pl) = app.playlists.get(app.playlist_sel).cloned() {
                    if let Ok(ts) = app.client.playlist_tracks(&pl.id).await {
                        app.playlist_tracks = ts;
                        app.playlist_track_sel = 0;
                    }
                }
            } else {
                move_idx(&mut app.playlist_track_sel, app.playlist_tracks.len(), 1);
            }
        }
        KeyCode::Enter => {
            if app.playlist_pane == 0 {
                if let Some(pl) = app.playlists.get(app.playlist_sel).cloned() {
                    if let Ok(ts) = app.client.playlist_tracks(&pl.id).await {
                        app.playlist_tracks = ts;
                        app.playlist_track_sel = 0;
                        app.playlist_pane = 1;
                    }
                }
            } else {
                let tracks = app.playlist_tracks.clone();
                app.play_tracks_from(tracks, app.playlist_track_sel).await;
            }
        }
        KeyCode::Char('N') => {
            app.modal = Modal::TextInput {
                title: " Create Playlist ".to_string(),
                prompt: "Playlist name:".to_string(),
                value: String::new(),
                action: InputAction::CreatePlaylist,
            };
        }
        KeyCode::Char('x') => {
            if let Some(pl) = app.playlists.get(app.playlist_sel).cloned() {
                if app.client.delete_playlist(&pl.id).await.is_ok() {
                    app.status_msg = format!("Deleted playlist '{}'", pl.name);
                    if let Ok(pls) = app.client.playlists().await {
                        app.playlists = pls;
                        app.playlist_sel = 0;
                    }
                }
            }
        }
        KeyCode::Char('d') => {
            if app.playlist_pane == 1 {
                if let (Some(pl), Some(tr)) = (
                    app.playlists.get(app.playlist_sel).cloned(),
                    app.playlist_tracks.get(app.playlist_track_sel).cloned(),
                ) {
                    if app.client.remove_track_from_playlist(&pl.id, &tr.id).await.is_ok() {
                        app.status_msg = format!("Removed '{}' from '{}'", tr.title, pl.name);
                        if let Ok(ts) = app.client.playlist_tracks(&pl.id).await {
                            app.playlist_tracks = ts;
                        }
                    }
                }
            }
        }
        KeyCode::Char('e') => {
            if let Ok(json) = app.client.export_playlists().await {
                let dest = crate::config::config_dir().join("playlists_backup.json");
                if tokio::fs::write(&dest, json).await.is_ok() {
                    app.status_msg = format!("Exported playlists to {}", dest.display());
                }
            }
        }
        _ => {}
    }
}

async fn handle_hearts_key(app: &mut App, key: KeyEvent) {
    let lens = [
        app.heart_details.tracks.len(),
        app.heart_details.albums.len(),
        app.heart_details.artists.len(),
        app.heart_details.playlists.len(),
    ];
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.heart_pane = (app.heart_pane + 3) % 4,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.heart_pane = (app.heart_pane + 1) % 4,
        KeyCode::Up | KeyCode::Char('k') => move_idx(&mut app.heart_sel[app.heart_pane], lens[app.heart_pane], -1),
        KeyCode::Down | KeyCode::Char('j') => move_idx(&mut app.heart_sel[app.heart_pane], lens[app.heart_pane], 1),
        KeyCode::Enter => match app.heart_pane {
            0 => {
                let tracks = app.heart_details.tracks.clone();
                app.play_tracks_from(tracks, app.heart_sel[0]).await;
            }
            1 => {
                if let Some(a) = app.heart_details.albums.get(app.heart_sel[1]).cloned() {
                    app.albums = vec![a.clone()];
                    app.album_sel = 0;
                    app.load_album_tracks(&a.id).await;
                    app.lib_pane = 2;
                    app.tab = Tab::Library;
                }
            }
            2 => {
                if let Some(a) = app.heart_details.artists.get(app.heart_sel[2]).cloned() {
                    app.load_artist_albums(&a.id).await;
                    app.lib_pane = 1;
                    app.tab = Tab::Library;
                }
            }
            _ => {}
        },
        KeyCode::Char('f') | KeyCode::Char('x') => match app.heart_pane {
            0 => {
                if let Some(t) = app.heart_details.tracks.get(app.heart_sel[0]).cloned() {
                    app.toggle_heart("track", &t.id, None).await;
                }
            }
            1 => {
                if let Some(a) = app.heart_details.albums.get(app.heart_sel[1]).cloned() {
                    app.toggle_heart("album", &a.id, None).await;
                }
            }
            2 => {
                if let Some(a) = app.heart_details.artists.get(app.heart_sel[2]).cloned() {
                    app.toggle_heart("artist", &a.id, None).await;
                }
            }
            _ => {
                if let Some(p) = app.heart_details.playlists.get(app.heart_sel[3]).cloned() {
                    app.toggle_heart("playlist", &p.id, None).await;
                }
            }
        },
        KeyCode::Char('e') => {
            if let Ok(json) = app.client.export_hearts().await {
                let dest = crate::config::config_dir().join("hearts_backup.json");
                if tokio::fs::write(&dest, json).await.is_ok() {
                    app.status_msg = format!("Exported hearts to {}", dest.display());
                }
            }
        }
        _ => {}
    }
}

async fn load_selected_podcast_episodes(app: &mut App) {
    let (feed_id, title) = if app.podcast_showing_search {
        match app.podcast_search.get(app.podcast_feed_sel) {
            Some(f) => (f.id_string(), f.title.clone()),
            None => return,
        }
    } else {
        match app.podcast_subs.get(app.podcast_feed_sel) {
            Some(s) => (s.feed_id.clone(), s.title.clone()),
            None => return,
        }
    };
    app.podcast_active_feed_title = title;
    match app.client.podcast_episodes(&feed_id).await {
        Ok(eps) => {
            let ids: Vec<String> = eps.iter().map(|e| e.id_string()).collect();
            if let Ok(prog) = app.client.podcast_progress_batch(&ids).await {
                app.podcast_progress.extend(prog);
            }
            app.podcast_episodes = eps;
            app.podcast_ep_sel = 0;
            app.podcast_pane = 1;
        }
        Err(e) => app.status_msg = format!("Failed to load episodes: {e}"),
    }
}

async fn handle_podcasts_key(app: &mut App, key: KeyEvent) {
    let feeds_len = if app.podcast_showing_search {
        app.podcast_search.len()
    } else {
        app.podcast_subs.len()
    };
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.podcast_pane = 0,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.podcast_pane = 1,
        KeyCode::Up | KeyCode::Char('k') => {
            if app.podcast_pane == 0 {
                move_idx(&mut app.podcast_feed_sel, feeds_len, -1);
            } else {
                move_idx(&mut app.podcast_ep_sel, app.podcast_episodes.len(), -1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.podcast_pane == 0 {
                move_idx(&mut app.podcast_feed_sel, feeds_len, 1);
            } else {
                move_idx(&mut app.podcast_ep_sel, app.podcast_episodes.len(), 1);
            }
        }
        KeyCode::Char('/') => {
            app.modal = Modal::TextInput {
                title: " Search Podcasts (PodcastIndex) ".to_string(),
                prompt: "Enter podcast search query:".to_string(),
                value: String::new(),
                action: InputAction::SearchPodcasts,
            };
        }
        KeyCode::Char('t') => {
            app.podcast_showing_search = !app.podcast_showing_search;
            app.podcast_feed_sel = 0;
        }
        KeyCode::Enter => {
            if app.podcast_pane == 0 {
                load_selected_podcast_episodes(app).await;
            } else if let Some(ep) = app.podcast_episodes.get(app.podcast_ep_sel).cloned() {
                app.play_podcast_episode(app.podcast_active_feed_title.clone(), ep)
                    .await;
            }
        }
        KeyCode::Char('s') => {
            if app.podcast_showing_search {
                if let Some(feed) = app.podcast_search.get(app.podcast_feed_sel).cloned() {
                    let fid = feed.id_string();
                    match app
                        .client
                        .podcast_subscribe(&fid, &feed.url, &feed.title, &feed.best_image())
                        .await
                    {
                        Ok(()) => {
                            app.status_msg = format!("Subscribed to '{}'", feed.title);
                            if let Ok(subs) = app.client.podcast_subscriptions().await {
                                app.podcast_subs = subs;
                            }
                        }
                        Err(e) => app.status_msg = format!("Subscribe failed: {e}"),
                    }
                }
            } else if let Some(sub) = app.podcast_subs.get(app.podcast_feed_sel).cloned() {
                if app.client.podcast_unsubscribe(&sub.feed_id).await.is_ok() {
                    app.status_msg = format!("Unsubscribed from '{}'", sub.title);
                    if let Ok(subs) = app.client.podcast_subscriptions().await {
                        app.podcast_subs = subs;
                    }
                }
            }
        }
        KeyCode::Char('m') => {
            if let Some(ep) = app.podcast_episodes.get(app.podcast_ep_sel).cloned() {
                let id = ep.id_string();
                let cur = app.podcast_progress.get(&id).map(|p| p.completed).unwrap_or(false);
                let pos = app.podcast_progress.get(&id).map(|p| p.position_ms).unwrap_or(0);
                if app.client.podcast_save_progress(&id, pos, !cur).await.is_ok() {
                    if let Ok(prog) = app.client.podcast_progress_batch(&[id]).await {
                        app.podcast_progress.extend(prog);
                    }
                }
            }
        }
        KeyCode::Char('e') => {
            if let Ok(opml) = app.client.podcast_export_opml().await {
                let dest = crate::config::config_dir().join("supernova_podcasts.opml");
                if tokio::fs::write(&dest, opml).await.is_ok() {
                    app.status_msg = format!("Exported OPML to {}", dest.display());
                }
            }
        }
        KeyCode::Char('I') => {
            app.modal = Modal::TextInput {
                title: " Import Podcast OPML ".to_string(),
                prompt: "Enter path to .opml file:".to_string(),
                value: String::new(),
                action: InputAction::ImportOpmlPath,
            };
        }
        _ => {}
    }
}

async fn handle_radio_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => app.radio_pane = 0,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => app.radio_pane = 1,
        KeyCode::Up | KeyCode::Char('k') => {
            if app.radio_pane == 0 {
                move_idx(&mut app.radio_sub_sel, app.radio_subs.len(), -1);
            } else {
                move_idx(&mut app.radio_search_sel, app.radio_search.len(), -1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.radio_pane == 0 {
                move_idx(&mut app.radio_sub_sel, app.radio_subs.len(), 1);
            } else {
                move_idx(&mut app.radio_search_sel, app.radio_search.len(), 1);
            }
        }
        KeyCode::Char('/') => {
            app.modal = Modal::TextInput {
                title: " Search Internet Radio ".to_string(),
                prompt: "Station name (or country:US):".to_string(),
                value: String::new(),
                action: InputAction::SearchRadio,
            };
        }
        KeyCode::Enter => {
            if app.radio_pane == 0 {
                if let Some(s) = app.radio_subs.get(app.radio_sub_sel).cloned() {
                    app.play_radio_station(s.name, s.station_id, s.url).await;
                }
            } else if let Some(s) = app.radio_search.get(app.radio_search_sel).cloned() {
                let url = s.stream_url().to_string();
                app.play_radio_station(s.name, s.stationuuid, url).await;
            }
        }
        KeyCode::Char('s') => {
            if app.radio_pane == 1 {
                if let Some(s) = app.radio_search.get(app.radio_search_sel).cloned() {
                    let url = s.stream_url().to_string();
                    if app
                        .client
                        .radio_subscribe(&s.stationuuid, &url, &s.name, &s.favicon)
                        .await
                        .is_ok()
                    {
                        app.status_msg = format!("Subscribed to radio '{}'", s.name);
                        if let Ok(subs) = app.client.radio_subscriptions().await {
                            app.radio_subs = subs;
                        }
                    }
                }
            } else if let Some(s) = app.radio_subs.get(app.radio_sub_sel).cloned() {
                if app.client.radio_unsubscribe(&s.station_id).await.is_ok() {
                    app.status_msg = format!("Removed radio '{}'", s.name);
                    if let Ok(subs) = app.client.radio_subscriptions().await {
                        app.radio_subs = subs;
                    }
                }
            }
        }
        KeyCode::Char('f') => {
            if app.radio_pane == 0 {
                if let Some(s) = app.radio_subs.get(app.radio_sub_sel).cloned() {
                    let meta = serde_json::json!({ "name": s.name, "url": s.url, "favicon": s.favicon });
                    app.toggle_heart("radio", &s.station_id, Some(meta)).await;
                }
            } else if let Some(s) = app.radio_search.get(app.radio_search_sel).cloned() {
                let meta = serde_json::json!({ "name": s.name, "url": s.stream_url(), "favicon": s.favicon });
                app.toggle_heart("radio", &s.stationuuid, Some(meta)).await;
            }
        }
        _ => {}
    }
}

async fn handle_lyrics_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.lyrics_scroll = app.lyrics_scroll.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => app.lyrics_scroll += 1,
        KeyCode::Char('r') => {
            if let Some(crate::app::NowPlayingItem::Track(t)) = app.now_playing.clone() {
                app.fetch_lyrics_for(&t).await;
            }
        }
        _ => {}
    }
}

async fn handle_plugins_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => move_idx(&mut app.plugin_sel, app.plugins.len(), -1),
        KeyCode::Down | KeyCode::Char('j') => move_idx(&mut app.plugin_sel, app.plugins.len(), 1),
        KeyCode::Char('r') => {
            if let Ok(p) = app.client.plugins().await {
                app.plugins = p;
            }
            if let Ok(sp) = app.client.scan_progress().await {
                app.scan_progress = sp;
            }
        }
        KeyCode::Char('P') => {
            app.subsonic_status = app
                .client
                .subsonic_ping_status()
                .await
                .unwrap_or_else(|e| format!("err: {e}"));
        }
        KeyCode::Char('l') => {
            match app.client.lastfm_auth_url("http://localhost/lastfm-cb").await {
                Ok(u) => {
                    app.lastfm_auth_url = Some(u);
                    app.status_msg = "Open Last.fm Auth URL in browser, then press 't' to paste token".to_string();
                }
                Err(e) => app.status_msg = format!("Last.fm auth URL error: {e}"),
            }
        }
        KeyCode::Char('t') => {
            app.modal = Modal::TextInput {
                title: " Last.fm Callback Token ".to_string(),
                prompt: "Paste the ?token=... value from Last.fm callback:".to_string(),
                value: String::new(),
                action: InputAction::LastFmToken,
            };
        }
        KeyCode::Char('F') => {
            let next = match app.tui_config.stream_format.as_deref() {
                None | Some("raw") => Some("mp3".to_string()),
                Some("mp3") => Some("opus".to_string()),
                Some("opus") => Some("aac".to_string()),
                Some("aac") => Some("ogg".to_string()),
                _ => None,
            };
            app.tui_config.stream_format = next;
            app.save_cfg();
        }
        KeyCode::Char('B') => {
            let next = match app.tui_config.stream_bitrate {
                None | Some(0) => Some(128),
                Some(128) => Some(192),
                Some(192) => Some(320),
                _ => None,
            };
            app.tui_config.stream_bitrate = next;
            app.save_cfg();
        }
        KeyCode::Char('S') => match app.client.scan_library().await {
            Ok(()) => {
                app.status_msg = "Library scan started".to_string();
                if let Ok(sp) = app.client.scan_progress().await {
                    app.scan_progress = sp;
                }
            }
            Err(e) => app.status_msg = format!("Scan failed: {e}"),
        },
        KeyCode::Char('R') => match app.client.reset_artists().await {
            Ok(()) => app.status_msg = "Artist enrichment reset triggered".to_string(),
            Err(e) => app.status_msg = format!("Reset failed: {e}"),
        },
        KeyCode::Char('T') => match app.client.run_autotagger().await {
            Ok(_) => app.status_msg = "Auto-Tagger job started".to_string(),
            Err(e) => app.status_msg = format!("Auto-Tagger failed: {e}"),
        },
        KeyCode::Char('v') => {
            app.preview_kind = PreviewKind::ArtistMerger;
            app.preview_scroll = 0;
            match app.client.preview_artist_merger().await {
                Ok(res) => {
                    app.preview_warning = res.warning;
                    app.preview_lines.clear();
                    for (i, group) in res.candidates.iter().enumerate() {
                        app.preview_lines.push(format!("Group #{}:", i + 1));
                        for c in group {
                            app.preview_lines.push(format!(
                                "  - {} (albums:{}, tracks:{}, hearts:{}, id:{})",
                                c.name, c.albums, c.tracks, c.hearts, c.id
                            ));
                        }
                    }
                    if app.preview_lines.is_empty() {
                        app.preview_lines.push("No artist merge candidates found.".to_string());
                    }
                }
                Err(e) => app.status_msg = format!("Preview failed: {e}"),
            }
        }
        KeyCode::Char('w') => {
            app.preview_kind = PreviewKind::AlbumMerger;
            app.preview_scroll = 0;
            match app.client.preview_album_merger().await {
                Ok(res) => {
                    app.preview_warning = res.warning;
                    app.preview_lines.clear();
                    for (i, group) in res.candidates.iter().enumerate() {
                        app.preview_lines.push(format!("Group #{}:", i + 1));
                        for c in group {
                            app.preview_lines.push(format!(
                                "  - {} - {} ({}) [tracks:{}, hearts:{}]",
                                c.artist, c.title, c.year, c.tracks, c.hearts
                            ));
                        }
                    }
                    if app.preview_lines.is_empty() {
                        app.preview_lines.push("No album merge candidates found.".to_string());
                    }
                }
                Err(e) => app.status_msg = format!("Preview failed: {e}"),
            }
        }
        KeyCode::Char('d') => {
            app.preview_kind = PreviewKind::Deduper;
            app.preview_scroll = 0;
            match app.client.preview_deduper().await {
                Ok(res) => {
                    app.preview_warning = res.warning;
                    app.preview_lines.clear();
                    for (i, group) in res.candidates.iter().enumerate() {
                        app.preview_lines.push(format!("Group #{}:", i + 1));
                        for c in group {
                            app.preview_lines.push(format!(
                                "  - {} - {} [{} {}kbps] ({}) {}",
                                c.artist, c.title, c.format, c.bitrate, c.album, c.file_path
                            ));
                        }
                    }
                    if app.preview_lines.is_empty() {
                        app.preview_lines.push("No duplicate candidates found.".to_string());
                    }
                }
                Err(e) => app.status_msg = format!("Preview failed: {e}"),
            }
        }
        KeyCode::PageDown => app.preview_scroll += 5,
        KeyCode::PageUp => app.preview_scroll = app.preview_scroll.saturating_sub(5),
        _ => {}
    }
}

async fn handle_change_password_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.modal = Modal::None,
        KeyCode::Tab | KeyCode::Down | KeyCode::BackTab | KeyCode::Up => {
            if let Modal::ChangePassword { field, .. } = &mut app.modal {
                *field = (*field + 1) % 2;
            }
        }
        KeyCode::Backspace => {
            if let Modal::ChangePassword { current, new, field, .. } = &mut app.modal {
                if *field == 0 {
                    current.pop();
                } else {
                    new.pop();
                }
            }
        }
        KeyCode::Char(c) => {
            if let Modal::ChangePassword { current, new, field, .. } = &mut app.modal {
                if *field == 0 {
                    current.push(c);
                } else {
                    new.push(c);
                }
            }
        }
        KeyCode::Enter => {
            let Modal::ChangePassword { current, new, .. } = &app.modal else {
                return;
            };
            let current = current.clone();
            let new = new.clone();
            match app.client.change_password(&current, &new).await {
                Ok(()) => {
                    app.modal = Modal::None;
                    app.status_msg = "Password updated".to_string();
                }
                Err(e) => {
                    if let Modal::ChangePassword { error, .. } = &mut app.modal {
                        *error = Some(e.to_string());
                    }
                }
            }
        }
        _ => {}
    }
}

fn cycle_stream_format(app: &mut App, forward: bool) {
    let formats = [None, Some("mp3"), Some("opus"), Some("aac"), Some("ogg")];
    let cur = formats
        .iter()
        .position(|f| *f == app.tui_config.stream_format.as_deref())
        .unwrap_or(0);
    let next = if forward {
        (cur + 1) % formats.len()
    } else {
        (cur + formats.len() - 1) % formats.len()
    };
    app.tui_config.stream_format = formats[next].map(str::to_string);
    app.save_cfg();
}

fn cycle_stream_bitrate(app: &mut App, forward: bool) {
    let bitrates = [None, Some(128), Some(192), Some(320)];
    let cur = bitrates
        .iter()
        .position(|b| *b == app.tui_config.stream_bitrate)
        .unwrap_or(0);
    let next = if forward {
        (cur + 1) % bitrates.len()
    } else {
        (cur + bitrates.len() - 1) % bitrates.len()
    };
    app.tui_config.stream_bitrate = bitrates[next];
    app.save_cfg();
}

async fn handle_settings_key(app: &mut App, key: KeyEvent) {
    const NUM_SETTINGS: usize = 8;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => move_idx(&mut app.settings_sel, NUM_SETTINGS, -1),
        KeyCode::Down | KeyCode::Char('j') => move_idx(&mut app.settings_sel, NUM_SETTINGS, 1),
        KeyCode::Left | KeyCode::Char('h') => match app.settings_sel {
            3 => cycle_stream_format(app, false),
            4 => cycle_stream_bitrate(app, false),
            5 => {
                app.tui_config.volume = app.tui_config.volume.saturating_sub(5);
                let _ = app.player.set_volume(app.tui_config.volume).await;
                app.save_cfg();
            }
            _ => {}
        },
        KeyCode::Right | KeyCode::Char('l') => match app.settings_sel {
            3 => cycle_stream_format(app, true),
            4 => cycle_stream_bitrate(app, true),
            5 => {
                app.tui_config.volume = (app.tui_config.volume + 5).min(100);
                let _ = app.player.set_volume(app.tui_config.volume).await;
                app.save_cfg();
            }
            _ => {}
        },
        KeyCode::Char('r') => {
            app.tui_config = load_config();
            let _ = save_config(&app.tui_config);
            let _ = app.player.set_volume(app.tui_config.volume).await;
            app.status_msg = "Reloaded config.json".to_string();
        }
        KeyCode::Char('x') => {
            if app.settings_sel == 6 {
                app.tui_config.lastfm_session_key = None;
                app.save_cfg();
                app.status_msg = "Cleared Last.fm session key".to_string();
            }
        }
        KeyCode::Char('t') => {
            app.modal = Modal::TextInput {
                title: " Last.fm Callback Token ".to_string(),
                prompt: "Paste the ?token=... value from Last.fm callback:".to_string(),
                value: String::new(),
                action: InputAction::LastFmToken,
            };
        }
        KeyCode::Enter => match app.settings_sel {
            0 => {
                app.modal = Modal::TextInput {
                    title: " Server URL ".to_string(),
                    prompt: "Enter Supernova server URL:".to_string(),
                    value: app.client.base_url.clone(),
                    action: InputAction::EditServerUrl,
                };
            }
            1 => {
                app.modal = Modal::Login {
                    url: app.client.base_url.clone(),
                    username: String::new(),
                    password: String::new(),
                    invite_code: String::new(),
                    is_register: false,
                    field: 1,
                    error: None,
                };
            }
            2 => {
                app.modal = Modal::ChangePassword {
                    current: String::new(),
                    new: String::new(),
                    field: 0,
                    error: None,
                };
            }
            3 => cycle_stream_format(app, true),
            4 => cycle_stream_bitrate(app, true),
            6 => match app.client.lastfm_auth_url("http://localhost/lastfm-cb").await {
                Ok(u) => {
                    app.lastfm_auth_url = Some(u);
                    app.status_msg = "Open Last.fm URL shown on right, then press 't'".to_string();
                }
                Err(e) => app.status_msg = format!("Last.fm auth URL error: {e}"),
            },
            7 => match app.client.health().await {
                Ok(v) => {
                    let st = v.get("status").and_then(|s| s.as_str()).unwrap_or("ok");
                    let ver = v.get("version").and_then(|s| s.as_str()).unwrap_or("?");
                    app.health_status = format!("{st} (v{ver})");
                }
                Err(e) => app.health_status = format!("Error: {e}"),
            },
            _ => {}
        },
        _ => {}
    }
}
