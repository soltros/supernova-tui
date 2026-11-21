use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::app::{App, Modal, NowPlayingItem, PreviewKind, Tab};
use crate::models::{format_ms, format_secs};

const ACCENT: Color = Color::Cyan;
const ACCENT2: Color = Color::Magenta;
const ACTIVE_BG: Color = Color::DarkGray;
const MUTED: Color = Color::Gray;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(8),    // Main content
            Constraint::Length(4), // Player bar
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);

    match app.tab {
        Tab::Dashboard => draw_dashboard(f, app, chunks[1]),
        Tab::Library => draw_library(f, app, chunks[1]),
        Tab::Search => draw_search(f, app, chunks[1]),
        Tab::Playlists => draw_playlists(f, app, chunks[1]),
        Tab::Hearts => draw_hearts(f, app, chunks[1]),
        Tab::Podcasts => draw_podcasts(f, app, chunks[1]),
        Tab::Radio => draw_radio(f, app, chunks[1]),
        Tab::Lyrics => draw_lyrics(f, app, chunks[1]),
        Tab::Plugins => draw_plugins(f, app, chunks[1]),
        Tab::Settings => draw_settings(f, app, chunks[1]),
    }

    draw_player_bar(f, app, chunks[2]);
    draw_status_bar(f, app, chunks[3]);

    match &app.modal {
        Modal::None => {}
        Modal::Help => draw_help_modal(f),
        Modal::Queue => draw_queue_modal(f, app),
        Modal::Login { .. } => draw_login_modal(f, app),
        Modal::TextInput { title, prompt, value, .. } => {
            draw_input_modal(f, title, prompt, value);
        }
        Modal::AddToPlaylist { track, selected } => {
            draw_add_to_playlist_modal(f, app, &track.title, *selected);
        }
        Modal::ChangePassword { current, new, field, error } => {
            draw_change_password_modal(f, current, new, *field, error.as_deref());
        }
    }
}

fn pane_block(title: &str, active: bool) -> Block<'_> {
    let border_style = if active {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
}

fn list_with_selection(items: Vec<ListItem<'_>>, selected: usize, active: bool) -> (List<'_>, ListState) {
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    let hl = if active {
        Style::default().bg(ACTIVE_BG).fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(ACCENT)
    };
    let list = List::new(items).highlight_style(hl).highlight_symbol("▶ ");
    (list, state)
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL.iter().map(|t| Line::from(t.title())).collect();
    let user_badge = app
        .user
        .as_ref()
        .map(|u| format!(" SUPERNOVA [{}{}] ", u.username, if u.is_admin { "*" } else { "" }))
        .unwrap_or_else(|| " SUPERNOVA ".to_string());

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(user_badge).border_style(Style::default().fg(ACCENT2)))
        .select(app.tab as usize)
        .style(Style::default().fg(MUTED))
        .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn draw_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(rows[0]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[1]);

    let album_items: Vec<ListItem> = app
        .dashboard
        .recently_added_albums
        .iter()
        .map(|a| ListItem::new(format!("{} - {} ({})", a.artist_name, a.title, a.release_year)))
        .collect();
    let (l0, mut s0) = list_with_selection(album_items, app.dash_sel[0], app.dash_pane == 0);
    f.render_stateful_widget(l0.block(pane_block(" Recently Added Albums ", app.dash_pane == 0)), top[0], &mut s0);

    let recent_items: Vec<ListItem> = app
        .dashboard
        .recently_played_tracks
        .iter()
        .map(|t| ListItem::new(format!("{} - {}", t.artist_name, t.title)))
        .collect();
    let (l1, mut s1) = list_with_selection(recent_items, app.dash_sel[1], app.dash_pane == 1);
    f.render_stateful_widget(l1.block(pane_block(" Recently Played ", app.dash_pane == 1)), top[1], &mut s1);

    let fav_items: Vec<ListItem> = app
        .dashboard
        .favorite_tracks
        .iter()
        .map(|t| ListItem::new(format!("♥ {} - {}", t.artist_name, t.title)))
        .collect();
    let (l2, mut s2) = list_with_selection(fav_items, app.dash_sel[2], app.dash_pane == 2);
    f.render_stateful_widget(l2.block(pane_block(" Favorite Tracks ", app.dash_pane == 2)), top[2], &mut s2);

    let radar_items: Vec<ListItem> = app
        .discovery
        .release_radar
        .iter()
        .map(|r| {
            let date = r.release_date.get(..10).unwrap_or(&r.release_date);
            ListItem::new(format!("{} - {} ({})", r.artist_name, r.collection_name, date))
        })
        .collect();
    let (l3, mut s3) = list_with_selection(radar_items, app.dash_sel[3], app.dash_pane == 3);
    f.render_stateful_widget(l3.block(pane_block(" Release Radar (Discovery) ", app.dash_pane == 3)), bottom[0], &mut s3);

    let sim_items: Vec<ListItem> = app
        .discovery
        .similar_artists
        .iter()
        .map(|s| {
            let mark = if s.id.is_empty() { "" } else { " [in library]" };
            ListItem::new(format!("{} (like {}){}", s.name, s.based_on, mark))
        })
        .collect();
    let (l4, mut s4) = list_with_selection(sim_items, app.dash_sel[4], app.dash_pane == 4);
    f.render_stateful_widget(l4.block(pane_block(" Similar Artists ", app.dash_pane == 4)), bottom[1], &mut s4);
}

fn draw_library(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(28),
            Constraint::Percentage(32),
            Constraint::Percentage(40),
        ])
        .split(area);

    let art_title = match &app.artist_letter {
        Some(l) => format!(" Artists [{l}] "),
        None => " Artists (L:filter, A:all albums) ".to_string(),
    };
    let art_items: Vec<ListItem> = app
        .artists
        .iter()
        .map(|a| {
            let h = if app.is_hearted("artist", &a.id) { "♥ " } else { "" };
            ListItem::new(format!("{h}{}", a.name))
        })
        .collect();
    let (l0, mut s0) = list_with_selection(art_items, app.artist_sel, app.lib_pane == 0);
    f.render_stateful_widget(l0.block(pane_block(&art_title, app.lib_pane == 0)), cols[0], &mut s0);

    let alb_items: Vec<ListItem> = app
        .albums
        .iter()
        .map(|a| {
            let h = if app.is_hearted("album", &a.id) { "♥ " } else { "" };
            let yr = if a.release_year > 0 { format!(" ({})", a.release_year) } else { String::new() };
            ListItem::new(format!("{h}{}{yr}", a.title))
        })
        .collect();
    let (l1, mut s1) = list_with_selection(alb_items, app.album_sel, app.lib_pane == 1);
    f.render_stateful_widget(l1.block(pane_block(" Albums ", app.lib_pane == 1)), cols[1], &mut s1);

    let tr_items: Vec<ListItem> = app
        .lib_tracks
        .iter()
        .map(|t| {
            let h = if app.is_hearted("track", &t.id) { "♥ " } else { "" };
            ListItem::new(format!(
                "{h}{:02}. {} [{}] ({})",
                t.track_number,
                t.title,
                t.format,
                format_ms(t.duration_ms)
            ))
        })
        .collect();
    let (l2, mut s2) = list_with_selection(tr_items, app.lib_track_sel, app.lib_pane == 2);
    f.render_stateful_widget(l2.block(pane_block(" Tracks (Enter:play, q:queue, f:heart, a:playlist, D:dl) ", app.lib_pane == 2)), cols[2], &mut s2);
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let query_box = Paragraph::new(format!("Query: {}   (press / to search)", app.search_query))
        .block(pane_block(" Search Library ", false));
    f.render_widget(query_box, rows[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ])
        .split(rows[1]);

    let a_items: Vec<ListItem> = app.search_results.artists.iter().map(|a| ListItem::new(a.name.clone())).collect();
    let (l0, mut s0) = list_with_selection(a_items, app.search_sel[0], app.search_pane == 0);
    f.render_stateful_widget(l0.block(pane_block(" Artists ", app.search_pane == 0)), cols[0], &mut s0);

    let al_items: Vec<ListItem> = app
        .search_results
        .albums
        .iter()
        .map(|a| ListItem::new(format!("{} - {}", a.artist_name, a.title)))
        .collect();
    let (l1, mut s1) = list_with_selection(al_items, app.search_sel[1], app.search_pane == 1);
    f.render_stateful_widget(l1.block(pane_block(" Albums ", app.search_pane == 1)), cols[1], &mut s1);

    let t_items: Vec<ListItem> = app
        .search_results
        .tracks
        .iter()
        .map(|t| ListItem::new(format!("{} - {} ({})", t.artist_name, t.title, format_ms(t.duration_ms))))
        .collect();
    let (l2, mut s2) = list_with_selection(t_items, app.search_sel[2], app.search_pane == 2);
    f.render_stateful_widget(l2.block(pane_block(" Tracks ", app.search_pane == 2)), cols[2], &mut s2);
}

fn draw_playlists(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    let pl_items: Vec<ListItem> = app.playlists.iter().map(|p| ListItem::new(p.name.clone())).collect();
    let (l0, mut s0) = list_with_selection(pl_items, app.playlist_sel, app.playlist_pane == 0);
    f.render_stateful_widget(
        l0.block(pane_block(" Playlists (N:new, x:del, e:export) ", app.playlist_pane == 0)),
        cols[0],
        &mut s0,
    );

    let tr_items: Vec<ListItem> = app
        .playlist_tracks
        .iter()
        .enumerate()
        .map(|(i, t)| ListItem::new(format!("{}. {} - {} ({})", i + 1, t.artist_name, t.title, format_ms(t.duration_ms))))
        .collect();
    let (l1, mut s1) = list_with_selection(tr_items, app.playlist_track_sel, app.playlist_pane == 1);
    f.render_stateful_widget(
        l1.block(pane_block(" Tracks (Enter:play, d:remove) ", app.playlist_pane == 1)),
        cols[1],
        &mut s1,
    );
}

fn draw_hearts(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(area);

    let t_items: Vec<ListItem> = app
        .heart_details
        .tracks
        .iter()
        .map(|t| ListItem::new(format!("♥ {} - {}", t.artist_name, t.title)))
        .collect();
    let (l0, mut s0) = list_with_selection(t_items, app.heart_sel[0], app.heart_pane == 0);
    f.render_stateful_widget(l0.block(pane_block(" Tracks (f:unheart, e:export) ", app.heart_pane == 0)), cols[0], &mut s0);

    let al_items: Vec<ListItem> = app
        .heart_details
        .albums
        .iter()
        .map(|a| ListItem::new(format!("♥ {} - {}", a.artist_name, a.title)))
        .collect();
    let (l1, mut s1) = list_with_selection(al_items, app.heart_sel[1], app.heart_pane == 1);
    f.render_stateful_widget(l1.block(pane_block(" Albums ", app.heart_pane == 1)), cols[1], &mut s1);

    let ar_items: Vec<ListItem> = app
        .heart_details
        .artists
        .iter()
        .map(|a| ListItem::new(format!("♥ {}", a.name)))
        .collect();
    let (l2, mut s2) = list_with_selection(ar_items, app.heart_sel[2], app.heart_pane == 2);
    f.render_stateful_widget(l2.block(pane_block(" Artists ", app.heart_pane == 2)), cols[2], &mut s2);

    let pl_items: Vec<ListItem> = app
        .heart_details
        .playlists
        .iter()
        .map(|p| ListItem::new(format!("♥ {}", p.name)))
        .collect();
    let (l3, mut s3) = list_with_selection(pl_items, app.heart_sel[3], app.heart_pane == 3);
    f.render_stateful_widget(l3.block(pane_block(" Playlists ", app.heart_pane == 3)), cols[3], &mut s3);
}

fn draw_podcasts(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    let left_title = if app.podcast_showing_search {
        " Search Results (/:search, s:sub, t:subs) "
    } else {
        " Subscriptions (/:search, s:unsub, e:opml, I:import) "
    };

    let feed_items: Vec<ListItem> = if app.podcast_showing_search {
        app.podcast_search
            .iter()
            .map(|p| ListItem::new(format!("{} ({})", p.title, p.author)))
            .collect()
    } else {
        app.podcast_subs
            .iter()
            .map(|p| ListItem::new(p.title.clone()))
            .collect()
    };
    let (l0, mut s0) = list_with_selection(feed_items, app.podcast_feed_sel, app.podcast_pane == 0);
    f.render_stateful_widget(l0.block(pane_block(left_title, app.podcast_pane == 0)), cols[0], &mut s0);

    let ep_items: Vec<ListItem> = app
        .podcast_episodes
        .iter()
        .map(|ep| {
            let id = ep.id_string();
            let prog = app.podcast_progress.get(&id);
            let badge = match prog {
                Some(p) if p.completed => "[✓] ".to_string(),
                Some(p) if p.position_ms > 0 => format!("[{}] ", format_ms(p.position_ms)),
                _ => String::new(),
            };
            ListItem::new(format!("{badge}{} ({})", ep.title, format_secs(ep.duration as f64)))
        })
        .collect();
    let ep_title = if app.podcast_active_feed_title.is_empty() {
        " Episodes (Enter:play, m:mark done) ".to_string()
    } else {
        format!(" {} - Episodes (Enter:play, m:done) ", app.podcast_active_feed_title)
    };
    let (l1, mut s1) = list_with_selection(ep_items, app.podcast_ep_sel, app.podcast_pane == 1);
    f.render_stateful_widget(l1.block(pane_block(&ep_title, app.podcast_pane == 1)), cols[1], &mut s1);
}

fn draw_radio(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let sub_items: Vec<ListItem> = app
        .radio_subs
        .iter()
        .map(|s| {
            let h = if app.is_hearted("radio", &s.station_id) { "♥ " } else { "" };
            ListItem::new(format!("{h}{}", s.name))
        })
        .collect();
    let (l0, mut s0) = list_with_selection(sub_items, app.radio_sub_sel, app.radio_pane == 0);
    f.render_stateful_widget(
        l0.block(pane_block(" Saved Radio Stations (Enter:play, s:unsub, f:heart) ", app.radio_pane == 0)),
        cols[0],
        &mut s0,
    );

    let search_items: Vec<ListItem> = app
        .radio_search
        .iter()
        .map(|s| ListItem::new(format!("{} [{} {}kbps] ({})", s.name, s.codec, s.bitrate, s.country)))
        .collect();
    let (l1, mut s1) = list_with_selection(search_items, app.radio_search_sel, app.radio_pane == 1);
    f.render_stateful_widget(
        l1.block(pane_block(" Radio Search (/:search, Enter:play, s:sub) ", app.radio_pane == 1)),
        cols[1],
        &mut s1,
    );
}

fn draw_lyrics(f: &mut Frame, app: &App, area: Rect) {
    let title = format!(" LRCLib Lyrics - {} (r:refresh) ", app.lyrics_status);
    let block = pane_block(&title, true);

    if !app.lrc_lines.is_empty() {
        let pos = app.playback.time_pos;
        let active_idx = app
            .lrc_lines
            .iter()
            .rposition(|l| l.time_sec <= pos + 0.25)
            .unwrap_or(0);

        let inner_h = area.height.saturating_sub(2) as usize;
        let start = active_idx.saturating_sub(inner_h / 2);
        let lines: Vec<Line> = app
            .lrc_lines
            .iter()
            .enumerate()
            .skip(start)
            .take(inner_h)
            .map(|(i, l)| {
                let txt = if l.text.is_empty() { "♪" } else { &l.text };
                if i == active_idx {
                    Line::from(Span::styled(
                        format!("▶ {txt}"),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ))
                } else {
                    Line::from(Span::styled(format!("  {txt}"), Style::default().fg(MUTED)))
                }
            })
            .collect();
        let p = Paragraph::new(lines).block(block).alignment(Alignment::Center);
        f.render_widget(p, area);
    } else if let Some(plain) = app.lyrics.as_ref().and_then(|l| l.plain_lyrics.as_ref()) {
        let p = Paragraph::new(plain.as_str())
            .block(block)
            .scroll((app.lyrics_scroll as u16, 0))
            .wrap(Wrap { trim: false });
        f.render_widget(p, area);
    } else {
        let p = Paragraph::new(app.lyrics_status.as_str())
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(p, area);
    }
}

fn draw_plugins(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[0]);

    let plug_items: Vec<ListItem> = app
        .plugins
        .iter()
        .map(|p| {
            let st = if p.enabled { "ON " } else { "OFF" };
            ListItem::new(format!("[{st}] {} ({})", p.name, p.id))
        })
        .collect();
    let (l0, mut s0) = list_with_selection(plug_items, app.plugin_sel, true);
    f.render_stateful_widget(l0.block(pane_block(" Registered Server Plugins ", true)), left_rows[0], &mut s0);

    let sel_desc = app
        .plugins
        .get(app.plugin_sel)
        .map(|p| p.description.as_str())
        .unwrap_or("");
    let lfm = app
        .tui_config
        .lastfm_session_key
        .as_deref()
        .map(|_| "Connected")
        .unwrap_or("Not connected");
    let fmt = app.tui_config.stream_format.as_deref().unwrap_or("raw");
    let br = app.tui_config.stream_bitrate.unwrap_or(0);
    let url_line = app
        .lastfm_auth_url
        .as_deref()
        .map(|u| format!("\nLast.fm Auth URL: {u}"))
        .unwrap_or_default();

    let info_text = format!(
        "{sel_desc}\n\n\
         Last.fm: {lfm}  (l:get auth url, t:enter token){url_line}\n\
         Stream: format={fmt} bitrate={br}  (F:cycle format, B:cycle bitrate)\n\
         Subsonic /rest/ping: {}  (P:test ping)\n\
         Scan Status: {} ({} files)  (S:scan, R:reset artists, T:autotagger)\n\
         Previews (Admin): (v:artistmerger, w:albummerger, d:deduper)",
        app.subsonic_status, app.scan_progress.status, app.scan_progress.files_scanned
    );
    let info = Paragraph::new(info_text)
        .block(pane_block(" Plugin Controls & Settings ", false))
        .wrap(Wrap { trim: true });
    f.render_widget(info, left_rows[1]);

    let prev_title = match app.preview_kind {
        PreviewKind::ArtistMerger => " Admin Preview: Artist Merger ",
        PreviewKind::AlbumMerger => " Admin Preview: Album Merger ",
        PreviewKind::Deduper => " Admin Preview: Deduper ",
    };
    let mut prev_text = Vec::new();
    if !app.preview_warning.is_empty() {
        prev_text.push(Line::from(Span::styled(
            format!("Warning: {}", app.preview_warning),
            Style::default().fg(Color::Yellow),
        )));
    }
    for line in app.preview_lines.iter().skip(app.preview_scroll) {
        prev_text.push(Line::from(line.as_str()));
    }
    if prev_text.is_empty() {
        prev_text.push(Line::from("Press v (Artist Merger), w (Album Merger), or d (Deduper) to inspect preview candidates."));
    }
    let prev = Paragraph::new(prev_text)
        .block(pane_block(prev_title, false))
        .wrap(Wrap { trim: false });
    f.render_widget(prev, cols[1]);
}

fn draw_player_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Now Playing ")
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    let (title_str, ratio, time_str) = match &app.now_playing {
        Some(item) => {
            let state = if app.playback.paused { "⏸" } else { "▶" };
            let pos = app.playback.time_pos;
            let dur = match item {
                NowPlayingItem::Track(t) if app.playback.duration <= 0.0 => t.duration_ms as f64 / 1000.0,
                _ => app.playback.duration,
            };
            let ratio = if dur > 0.0 { (pos / dur).clamp(0.0, 1.0) } else { 0.0 };
            let q_info = app
                .queue_idx
                .map(|i| format!(" [Q {}/{}]", i + 1, app.queue.len()))
                .unwrap_or_default();
            (
                format!("{state} {} — {}{q_info}  Vol:{}%", item.title(), item.subtitle(), app.tui_config.volume),
                ratio,
                format!("{} / {}", format_secs(pos), format_secs(dur)),
            )
        }
        None => ("Idle — select a track, podcast, or radio station".to_string(), 0.0, "0:00 / 0:00".to_string()),
    };

    f.render_widget(Paragraph::new(title_str), rows[0]);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(ACCENT).bg(Color::Black))
        .ratio(ratio)
        .label(time_str);
    f.render_widget(gauge, rows[1]);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let p = Paragraph::new(format!(" {}   [?:Help  u:Queue  0:Settings  Space:Pause  Q:Quit]", app.status_msg))
        .style(Style::default().fg(MUTED));
    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}

fn draw_help_modal(f: &mut Frame) {
    let area = centered_rect(72, 78, f.area());
    f.render_widget(Clear, area);
    let text = "\
Global:
  1..9, 0    Switch tabs (1-9:Views, 0:Settings)
  Space      Play / Pause
  > / <      Next / Previous track in queue
  [ / ]      Seek -10s / +10s
  + / -      Volume up / down
  u          Toggle Queue modal
  C          Logout / Switch Server
  ?          Toggle Help
  Q / Ctrl+C Quit

Navigation:
  Up/Down, j/k   Move selection
  Left/Right,h/l Switch pane
  Enter          Play / Open

Library / Playlists / Hearts:
  f          Toggle Heart (track/album/artist/radio)
  q          Add track to queue
  a          Add track to playlist
  D          Download track to ~/Downloads
  N / x / d  Create playlist / Delete playlist / Remove track
  e          Export Hearts / Playlists / Podcast OPML

Plugins (Podcasts / Radio / Lyrics / Plugins):
  /          Search (Library, Podcasts, Radio)
  s          Subscribe / Unsubscribe (Podcasts, Radio)
  m          Mark podcast episode completed
  l / t      Last.fm auth URL / exchange callback token
  S / R / T  Admin: Scan library / Reset artists / Run Auto-Tagger
  v / w / d  Admin: Preview ArtistMerger / AlbumMerger / Deduper";
    let p = Paragraph::new(text)
        .block(pane_block(" Supernova TUI Help (Esc to close) ", true))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_queue_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(65, 65, f.area());
    f.render_widget(Clear, area);
    let items: Vec<ListItem> = app
        .queue
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let cur = if app.queue_idx == Some(i) { "▶ " } else { "  " };
            ListItem::new(format!("{cur}{}. {} - {}", i + 1, t.artist_name, t.title))
        })
        .collect();
    let list = List::new(items).block(pane_block(" Playback Queue (Esc to close, c:clear) ", true));
    f.render_widget(list, area);
}

fn draw_login_modal(f: &mut Frame, app: &App) {
    let Modal::Login {
        url,
        username,
        password,
        invite_code,
        is_register,
        field,
        error,
    } = &app.modal
    else {
        return;
    };

    let area = centered_rect(55, 48, f.area());
    f.render_widget(Clear, area);
    let title = if *is_register {
        " Supernova Register (Tab:next, Ctrl+R:mode, Enter:submit) "
    } else {
        " Supernova Login (Tab:next, Ctrl+R:register, Enter:submit) "
    };

    let masked = "*".repeat(password.len());
    let f0 = if *field == 0 { ">" } else { " " };
    let f1 = if *field == 1 { ">" } else { " " };
    let f2 = if *field == 2 { ">" } else { " " };
    let f3 = if *field == 3 { ">" } else { " " };

    let mut lines = vec![
        Line::from(format!("{f0} Server URL:  {url}")),
        Line::from(format!("{f1} Username:    {username}")),
        Line::from(format!("{f2} Password:    {masked}")),
    ];
    if *is_register {
        lines.push(Line::from(format!("{f3} Invite Code: {invite_code}")));
    }
    if let Some(err) = error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red))));
    }

    let p = Paragraph::new(lines)
        .block(pane_block(title, true))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn draw_input_modal(f: &mut Frame, title: &str, prompt: &str, value: &str) {
    let area = centered_rect(55, 22, f.area());
    f.render_widget(Clear, area);
    let p = Paragraph::new(format!("{prompt}\n\n> {value}_"))
        .block(pane_block(title, true))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_add_to_playlist_modal(f: &mut Frame, app: &App, track_title: &str, selected: usize) {
    let area = centered_rect(50, 50, f.area());
    f.render_widget(Clear, area);
    let items: Vec<ListItem> = app
        .playlists
        .iter()
        .map(|p| ListItem::new(p.name.clone()))
        .collect();
    let (list, mut state) = list_with_selection(items, selected, true);
    let title = format!(" Add '{track_title}' to Playlist (Enter/Esc) ");
    f.render_stateful_widget(list.block(pane_block(&title, true)), area, &mut state);
}

fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let url_str = app.tui_config.url.as_deref().unwrap_or(&app.client.base_url);
    let user_str = app
        .user
        .as_ref()
        .map(|u| format!("{} (admin: {})", u.username, u.is_admin))
        .unwrap_or_else(|| "Not signed in".to_string());
    let fmt_str = app.tui_config.stream_format.as_deref().unwrap_or("raw (original)");
    let br_str = match app.tui_config.stream_bitrate {
        Some(b) if b > 0 => format!("{b} kbps"),
        _ => "original".to_string(),
    };
    let vol_str = format!("{}%", app.tui_config.volume);
    let lfm_str = app
        .tui_config
        .lastfm_session_key
        .as_deref()
        .map(|k| {
            let preview: String = k.chars().take(6).collect();
            format!("Connected ({preview}...)")
        })
        .unwrap_or_else(|| "Not connected".to_string());

    let items = vec![
        ListItem::new(format!("Server URL:        {url_str}   [Enter: edit]")),
        ListItem::new(format!("Account:           {user_str}   [Enter: login/switch]")),
        ListItem::new("Password:          ********   [Enter: change password]".to_string()),
        ListItem::new(format!("Stream Format:     {fmt_str}   [Left/Right/Enter: cycle]")),
        ListItem::new(format!("Stream Bitrate:    {br_str}   [Left/Right/Enter: cycle]")),
        ListItem::new(format!("Default Volume:    {vol_str}   [Left/Right: -5% / +5%]")),
        ListItem::new(format!("Last.fm Session:   {lfm_str}   [Enter: auth URL, t: token, x: clear]")),
        ListItem::new(format!("Server Health:     {}   [Enter: check /api/health]", app.health_status)),
    ];

    let (list, mut state) = list_with_selection(items, app.settings_sel, true);
    f.render_stateful_widget(
        list.block(pane_block(" Client Settings (~/.config/supernova/config.json) ", true)),
        cols[0],
        &mut state,
    );

    let cfg_path = crate::config::config_path();
    let lfm_url_line = app
        .lastfm_auth_url
        .as_deref()
        .map(|u| format!("\n\nLast.fm Auth URL:\n{u}\n(Open in browser, then press 't' to paste token)"))
        .unwrap_or_default();

    let details = format!(
        "Config File:\n{}\n\n\
         All changes made here are automatically saved to config.json.\n\n\
         Keys:\n\
           Up/Down, j/k   Select setting\n\
           Enter          Edit / Activate\n\
           Left / Right   Adjust value\n\
           r              Reload config.json from disk\n\
           x              Clear Last.fm session key\n\
           t              Paste Last.fm callback token{lfm_url_line}",
        cfg_path.display()
    );
    let p = Paragraph::new(details)
        .block(pane_block(" Details ", false))
        .wrap(Wrap { trim: true });
    f.render_widget(p, cols[1]);
}

fn draw_change_password_modal(
    f: &mut Frame,
    current: &str,
    new: &str,
    field: usize,
    error: Option<&str>,
) {
    let area = centered_rect(50, 35, f.area());
    f.render_widget(Clear, area);
    let f0 = if field == 0 { ">" } else { " " };
    let f1 = if field == 1 { ">" } else { " " };
    let mut lines = vec![
        Line::from(format!("{f0} Current Password: {}", "*".repeat(current.len()))),
        Line::from(format!("{f1} New Password:     {}", "*".repeat(new.len()))),
    ];
    if let Some(err) = error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(err, Style::default().fg(Color::Red))));
    }
    let p = Paragraph::new(lines)
        .block(pane_block(" Change Password (Tab:next, Enter:submit, Esc:cancel) ", true))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}
