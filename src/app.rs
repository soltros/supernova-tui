use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::client::SupernovaClient;
use crate::config::{save_config, ClientConfig};
use crate::models::*;
use crate::player::{MpvPlayer, PlaybackStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard = 0,
    Library = 1,
    Search = 2,
    Playlists = 3,
    Hearts = 4,
    Podcasts = 5,
    Radio = 6,
    Lyrics = 7,
    Plugins = 8,
    Settings = 9,
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Dashboard,
        Tab::Library,
        Tab::Search,
        Tab::Playlists,
        Tab::Hearts,
        Tab::Podcasts,
        Tab::Radio,
        Tab::Lyrics,
        Tab::Plugins,
        Tab::Settings,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Dashboard => "1:Home",
            Tab::Library => "2:Library",
            Tab::Search => "3:Search",
            Tab::Playlists => "4:Playlists",
            Tab::Hearts => "5:Hearts",
            Tab::Podcasts => "6:Podcasts",
            Tab::Radio => "7:Radio",
            Tab::Lyrics => "8:Lyrics",
            Tab::Plugins => "9:Plugins",
            Tab::Settings => "0:Settings",
        }
    }
}

#[derive(Debug, Clone)]
pub enum NowPlayingItem {
    Track(Track),
    Podcast {
        feed_title: String,
        episode: PodcastEpisode,
    },
    Radio {
        name: String,
        station_id: String,
        url: String,
    },
}

impl NowPlayingItem {
    pub fn title(&self) -> &str {
        match self {
            NowPlayingItem::Track(t) => &t.title,
            NowPlayingItem::Podcast { episode, .. } => &episode.title,
            NowPlayingItem::Radio { name, .. } => name,
        }
    }

    pub fn subtitle(&self) -> String {
        match self {
            NowPlayingItem::Track(t) => {
                if t.artist_name.is_empty() {
                    "Unknown Artist".to_string()
                } else {
                    t.artist_name.clone()
                }
            }
            NowPlayingItem::Podcast { feed_title, .. } => feed_title.clone(),
            NowPlayingItem::Radio { .. } => "Live Internet Radio".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    Help,
    Queue,
    Login {
        url: String,
        username: String,
        password: String,
        invite_code: String,
        is_register: bool,
        field: usize, // 0=url, 1=user, 2=pass, 3=invite
        error: Option<String>,
    },
    TextInput {
        title: String,
        prompt: String,
        value: String,
        action: InputAction,
    },
    AddToPlaylist {
        track: Track,
        selected: usize,
    },
    ChangePassword {
        current: String,
        new: String,
        field: usize,
        error: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum InputAction {
    SearchLibrary,
    CreatePlaylist,
    SearchPodcasts,
    ImportOpmlPath,
    SearchRadio,
    LastFmToken,
    ArtistLetter,
    EditServerUrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    ArtistMerger,
    AlbumMerger,
    Deduper,
}

pub struct App {
    pub client: SupernovaClient,
    pub player: MpvPlayer,
    pub tui_config: ClientConfig,
    pub user: Option<User>,

    pub tab: Tab,
    pub modal: Modal,
    pub status_msg: String,
    pub should_quit: bool,

    // Playback & Queue
    pub queue: Vec<Track>,
    pub queue_idx: Option<usize>,
    pub now_playing: Option<NowPlayingItem>,
    pub playback: PlaybackStatus,
    pub scrobbled_current: bool,
    pub play_started_at: Option<Instant>,
    pub last_podcast_save: Instant,
    pub last_poll: Instant,

    // Dashboard (Tab 1)
    pub dashboard: DashboardData,
    pub discovery: DiscoveryData,
    pub dash_pane: usize, // 0=recent_albums, 1=recent_tracks, 2=fav_tracks, 3=release_radar, 4=similar_artists
    pub dash_sel: [usize; 5],

    // Library (Tab 2)
    pub artists: Vec<Artist>,
    pub artist_sel: usize,
    pub artist_letter: Option<String>,
    pub albums: Vec<Album>,
    pub album_sel: usize,
    pub lib_tracks: Vec<Track>,
    pub lib_track_sel: usize,
    pub lib_pane: usize, // 0=artists, 1=albums, 2=tracks

    // Search (Tab 3)
    pub search_query: String,
    pub search_results: SearchResults,
    pub search_pane: usize, // 0=artists, 1=albums, 2=tracks
    pub search_sel: [usize; 3],

    // Playlists (Tab 4)
    pub playlists: Vec<Playlist>,
    pub playlist_sel: usize,
    pub playlist_tracks: Vec<Track>,
    pub playlist_track_sel: usize,
    pub playlist_pane: usize, // 0=playlists, 1=tracks

    // Hearts (Tab 5)
    pub heart_ids: HashSet<(String, String)>,
    pub heart_details: HeartDetails,
    pub heart_pane: usize, // 0=tracks, 1=albums, 2=artists, 3=playlists
    pub heart_sel: [usize; 4],

    // Podcasts (Tab 6)
    pub podcast_subs: Vec<PodcastSubscription>,
    pub podcast_search: Vec<PodcastFeed>,
    pub podcast_showing_search: bool,
    pub podcast_feed_sel: usize,
    pub podcast_episodes: Vec<PodcastEpisode>,
    pub podcast_ep_sel: usize,
    pub podcast_progress: PodcastProgressMap,
    pub podcast_pane: usize, // 0=feeds, 1=episodes
    pub podcast_active_feed_title: String,

    // Radio (Tab 7)
    pub radio_subs: Vec<RadioSubscription>,
    pub radio_search: Vec<RadioStation>,
    pub radio_pane: usize, // 0=subscriptions, 1=search
    pub radio_sub_sel: usize,
    pub radio_search_sel: usize,

    // Lyrics (Tab 8)
    pub lyrics: Option<LyricsResponse>,
    pub lrc_lines: Vec<LrcLine>,
    pub lyrics_status: String,
    pub lyrics_scroll: usize,

    // Plugins & Admin (Tab 9)
    pub plugins: Vec<PluginInfo>,
    pub plugin_sel: usize,
    pub scan_progress: ScanProgress,
    pub subsonic_status: String,
    pub lastfm_auth_url: Option<String>,
    pub preview_kind: PreviewKind,
    pub preview_warning: String,
    pub preview_lines: Vec<String>,
    pub preview_scroll: usize,

    // Settings (Tab 0)
    pub settings_sel: usize,
    pub health_status: String,
}

impl App {
    pub fn new(base_url: String, token: Option<String>, tui_config: ClientConfig) -> anyhow::Result<Self> {
        let client = SupernovaClient::new(base_url.clone(), token.clone())?;
        let modal = if token.is_none() {
            Modal::Login {
                url: base_url,
                username: String::new(),
                password: String::new(),
                invite_code: String::new(),
                is_register: false,
                field: 1,
                error: None,
            }
        } else {
            Modal::None
        };

        Ok(Self {
            client,
            player: MpvPlayer::new(),
            tui_config,
            user: None,
            tab: Tab::Dashboard,
            modal,
            status_msg: "Ready. Press ? for help.".to_string(),
            should_quit: false,

            queue: Vec::new(),
            queue_idx: None,
            now_playing: None,
            playback: PlaybackStatus::default(),
            scrobbled_current: false,
            play_started_at: None,
            last_podcast_save: Instant::now(),
            last_poll: Instant::now(),

            dashboard: DashboardData::default(),
            discovery: DiscoveryData::default(),
            dash_pane: 0,
            dash_sel: [0; 5],

            artists: Vec::new(),
            artist_sel: 0,
            artist_letter: None,
            albums: Vec::new(),
            album_sel: 0,
            lib_tracks: Vec::new(),
            lib_track_sel: 0,
            lib_pane: 0,

            search_query: String::new(),
            search_results: SearchResults::default(),
            search_pane: 2,
            search_sel: [0; 3],

            playlists: Vec::new(),
            playlist_sel: 0,
            playlist_tracks: Vec::new(),
            playlist_track_sel: 0,
            playlist_pane: 0,

            heart_ids: HashSet::new(),
            heart_details: HeartDetails::default(),
            heart_pane: 0,
            heart_sel: [0; 4],

            podcast_subs: Vec::new(),
            podcast_search: Vec::new(),
            podcast_showing_search: false,
            podcast_feed_sel: 0,
            podcast_episodes: Vec::new(),
            podcast_ep_sel: 0,
            podcast_progress: HashMap::new(),
            podcast_pane: 0,
            podcast_active_feed_title: String::new(),

            radio_subs: Vec::new(),
            radio_search: Vec::new(),
            radio_pane: 0,
            radio_sub_sel: 0,
            radio_search_sel: 0,

            lyrics: None,
            lrc_lines: Vec::new(),
            lyrics_status: "No track playing".to_string(),
            lyrics_scroll: 0,

            plugins: Vec::new(),
            plugin_sel: 0,
            scan_progress: ScanProgress::default(),
            subsonic_status: "unchecked".to_string(),
            lastfm_auth_url: None,
            preview_kind: PreviewKind::ArtistMerger,
            preview_warning: String::new(),
            preview_lines: Vec::new(),
            preview_scroll: 0,

            settings_sel: 0,
            health_status: "Not checked".to_string(),
        })
    }

    pub fn home_favorites(&self) -> &[Track] {
        if !self.heart_details.tracks.is_empty() {
            &self.heart_details.tracks
        } else {
            &self.dashboard.favorite_tracks
        }
    }

    pub fn has_discovery(&self) -> bool {
        !self.discovery.release_radar.is_empty() || !self.discovery.similar_artists.is_empty()
    }

    pub fn mpris_state(&self) -> crate::mpris::MprisState {
        match &self.now_playing {
            None => crate::mpris::MprisState {
                playback_status: "Stopped".to_string(),
                volume: self.tui_config.volume as f64 / 100.0,
                ..Default::default()
            },
            Some(item) => {
                let status = if self.playback.paused {
                    "Paused"
                } else {
                    "Playing"
                };
                let (track_id, album, art_url, dur_s) = match item {
                    NowPlayingItem::Track(t) => {
                        let alb = self
                            .albums
                            .iter()
                            .find(|a| a.id == t.album_id)
                            .map(|a| a.title.clone())
                            .unwrap_or_default();
                        let art = if t.album_id.is_empty() {
                            String::new()
                        } else {
                            format!("{}/api/art/album/{}", self.client.base_url, t.album_id)
                        };
                        let d = if self.playback.duration > 0.0 {
                            self.playback.duration
                        } else {
                            t.duration_ms as f64 / 1000.0
                        };
                        (t.id.clone(), alb, art, d)
                    }
                    NowPlayingItem::Podcast { feed_title, episode } => (
                        format!("pod_{}", episode.id_string()),
                        feed_title.clone(),
                        String::new(),
                        self.playback.duration.max(episode.duration as f64),
                    ),
                    NowPlayingItem::Radio { station_id, .. } => (
                        format!("radio_{station_id}"),
                        "Internet Radio".to_string(),
                        String::new(),
                        0.0,
                    ),
                };
                crate::mpris::MprisState {
                    playback_status: status.to_string(),
                    track_id,
                    title: item.title().to_string(),
                    artist: item.subtitle(),
                    album,
                    art_url,
                    length_us: (dur_s * 1_000_000.0) as i64,
                    position_us: (self.playback.time_pos * 1_000_000.0) as i64,
                    volume: self.tui_config.volume as f64 / 100.0,
                }
            }
        }
    }

    pub async fn init_authenticated(&mut self) {
        match self.client.me().await {
            Ok(u) => {
                self.status_msg = format!("Signed in as {} ({})", u.username, if u.is_admin { "admin" } else { "user" });
                self.user = Some(u);
                self.modal = Modal::None;
                let _ = self.player.set_volume(self.tui_config.volume).await;
                self.refresh_initial().await;
            }
            Err(e) => {
                self.status_msg = format!("Auth failed: {e}");
                self.modal = Modal::Login {
                    url: self.client.base_url.clone(),
                    username: String::new(),
                    password: String::new(),
                    invite_code: String::new(),
                    is_register: false,
                    field: 1,
                    error: Some(format!("Session invalid or expired: {e}")),
                };
            }
        }
    }

    pub async fn refresh_initial(&mut self) {
        if let Ok(d) = self.client.dashboard().await {
            self.dashboard = d;
        }
        if let Ok(disc) = self.client.discovery().await {
            self.discovery = disc;
        }
        if let Ok(arts) = self.client.artists(200, 0, None).await {
            self.artists = arts;
            if let Some(first) = self.artists.first().cloned() {
                self.load_artist_albums(&first.id).await;
            }
        }
        if let Ok(pls) = self.client.playlists().await {
            self.playlists = pls;
            if let Some(first) = self.playlists.first().cloned() {
                if let Ok(ts) = self.client.playlist_tracks(&first.id).await {
                    self.playlist_tracks = ts;
                }
            }
        }
        self.refresh_hearts().await;
        if let Ok(plugs) = self.client.plugins().await {
            self.plugins = plugs;
        }
        if let Ok(ps) = self.client.podcast_subscriptions().await {
            self.podcast_subs = ps;
        }
        if let Ok(rs) = self.client.radio_subscriptions().await {
            self.radio_subs = rs;
        }
    }

    pub async fn refresh_hearts(&mut self) {
        if let Ok(hs) = self.client.hearts().await {
            self.heart_ids = hs.into_iter().map(|h| (h.entity_type, h.entity_id)).collect();
        }
        if let Ok(hd) = self.client.heart_details().await {
            self.heart_details = hd;
        }
    }

    pub fn is_hearted(&self, entity_type: &str, entity_id: &str) -> bool {
        self.heart_ids
            .contains(&(entity_type.to_string(), entity_id.to_string()))
    }

    pub async fn toggle_heart(
        &mut self,
        entity_type: &str,
        entity_id: &str,
        metadata: Option<serde_json::Value>,
    ) {
        if entity_id.is_empty() {
            return;
        }
        if self.is_hearted(entity_type, entity_id) {
            match self.client.remove_heart(entity_type, entity_id).await {
                Ok(()) => {
                    self.status_msg = format!("Removed {entity_type} from hearts");
                    self.refresh_hearts().await;
                }
                Err(e) => self.status_msg = format!("Failed to unheart: {e}"),
            }
        } else {
            match self.client.add_heart(entity_type, entity_id, metadata).await {
                Ok(()) => {
                    self.status_msg = format!("Added {entity_type} to hearts");
                    self.refresh_hearts().await;
                }
                Err(e) => self.status_msg = format!("Failed to heart: {e}"),
            }
        }
    }

    pub async fn load_artist_albums(&mut self, artist_id: &str) {
        if let Ok(albs) = self.client.albums(200, 0, Some(artist_id)).await {
            self.albums = albs;
            self.album_sel = 0;
            if let Some(first) = self.albums.first().cloned() {
                self.load_album_tracks(&first.id).await;
            } else {
                self.lib_tracks.clear();
                self.lib_track_sel = 0;
            }
        }
    }

    pub async fn load_album_tracks(&mut self, album_id: &str) {
        if let Ok(ts) = self.client.tracks(Some(album_id), None, 200, 0).await {
            self.lib_tracks = ts;
            self.lib_track_sel = 0;
        }
    }

    pub async fn play_tracks_from(&mut self, tracks: Vec<Track>, start_idx: usize) {
        if tracks.is_empty() || start_idx >= tracks.len() {
            return;
        }
        self.queue = tracks;
        self.queue_idx = Some(start_idx);
        self.play_current_queue_track().await;
    }

    pub async fn play_current_queue_track(&mut self) {
        let Some(idx) = self.queue_idx else { return };
        let Some(track) = self.queue.get(idx).cloned() else {
            return;
        };
        let fmt = self.tui_config.stream_format.as_deref();
        let br = self.tui_config.stream_bitrate;
        match self.client.stream_url(&track.id, fmt, br).await {
            Ok(url) => {
                if let Err(e) = self.player.play_url(&url, None).await {
                    self.status_msg = format!("Player error: {e}");
                    return;
                }
                let _ = self.player.set_volume(self.tui_config.volume).await;
                self.now_playing = Some(NowPlayingItem::Track(track.clone()));
                self.scrobbled_current = false;
                self.play_started_at = Some(Instant::now());
                self.status_msg = format!("Playing: {} - {}", track.artist_name, track.title);

                // Last.fm Now Playing
                if let Some(sk) = self.tui_config.lastfm_session_key.clone() {
                    let client = self.client.clone();
                    let artist = track.artist_name.clone();
                    let title = track.title.clone();
                    tokio::spawn(async move {
                        let _ = client.lastfm_now_playing(&sk, &artist, &title).await;
                    });
                }

                // Fetch LRCLib lyrics
                self.fetch_lyrics_for(&track).await;
            }
            Err(e) => {
                self.status_msg = format!("Failed to get stream ticket: {e}");
            }
        }
    }

    pub async fn fetch_lyrics_for(&mut self, track: &Track) {
        self.lyrics = None;
        self.lrc_lines.clear();
        self.lyrics_scroll = 0;
        self.lyrics_status = "Loading lyrics from LRCLib...".to_string();

        let album_title = self
            .albums
            .iter()
            .find(|a| a.id == track.album_id)
            .map(|a| a.title.clone())
            .unwrap_or_default();
        let dur_sec = track.duration_ms / 1000;

        match self
            .client
            .get_lyrics(&track.artist_name, &track.title, &album_title, dur_sec)
            .await
        {
            Ok(Some(res)) => {
                if let Some(synced) = &res.synced_lyrics {
                    self.lrc_lines = parse_lrc(synced);
                }
                self.lyrics_status = if res.instrumental {
                    "Instrumental".to_string()
                } else if !self.lrc_lines.is_empty() {
                    format!("Synced lyrics ({} lines)", self.lrc_lines.len())
                } else if res.plain_lyrics.is_some() {
                    "Plain lyrics".to_string()
                } else {
                    "No lyrics available".to_string()
                };
                self.lyrics = Some(res);
            }
            Ok(None) => {
                self.lyrics_status = "Lyrics not found on LRCLib".to_string();
            }
            Err(e) => {
                self.lyrics_status = format!("Lyrics error: {e}");
            }
        }
    }

    pub async fn play_next(&mut self) {
        if let Some(idx) = self.queue_idx {
            if idx + 1 < self.queue.len() {
                self.queue_idx = Some(idx + 1);
                self.play_current_queue_track().await;
            }
        }
    }

    pub async fn play_prev(&mut self) {
        if let Some(idx) = self.queue_idx {
            if idx > 0 {
                self.queue_idx = Some(idx - 1);
                self.play_current_queue_track().await;
            }
        }
    }

    pub async fn play_podcast_episode(&mut self, feed_title: String, ep: PodcastEpisode) {
        if ep.enclosure_url.is_empty() {
            self.status_msg = "Episode has no audio enclosure URL".to_string();
            return;
        }
        let ep_id = ep.id_string();
        let start_secs = self
            .podcast_progress
            .get(&ep_id)
            .filter(|p| !p.completed && p.position_ms > 2000)
            .map(|p| p.position_ms as f64 / 1000.0);

        match self.player.play_url(&ep.enclosure_url, start_secs).await {
            Ok(()) => {
                let _ = self.player.set_volume(self.tui_config.volume).await;
                self.now_playing = Some(NowPlayingItem::Podcast {
                    feed_title,
                    episode: ep.clone(),
                });
                self.play_started_at = Some(Instant::now());
                self.last_podcast_save = Instant::now();
                self.status_msg = format!("Playing episode: {}", ep.title);
            }
            Err(e) => self.status_msg = format!("Failed to play episode: {e}"),
        }
    }

    pub async fn play_radio_station(&mut self, name: String, station_id: String, url: String) {
        match self.player.play_url(&url, None).await {
            Ok(()) => {
                let _ = self.player.set_volume(self.tui_config.volume).await;
                self.now_playing = Some(NowPlayingItem::Radio {
                    name: name.clone(),
                    station_id,
                    url,
                });
                self.play_started_at = Some(Instant::now());
                self.status_msg = format!("Streaming radio: {name}");
            }
            Err(e) => self.status_msg = format!("Failed to stream radio: {e}"),
        }
    }

    pub async fn tick(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(350) {
            return;
        }
        self.last_poll = Instant::now();

        if self.now_playing.is_some() {
            self.playback = self.player.poll_status().await;

            // Check scrobble threshold for tracks (after 50% or 4 min, min 10s)
            if let Some(NowPlayingItem::Track(track)) = &self.now_playing {
                if !self.scrobbled_current {
                    let dur = self.playback.duration.max(track.duration_ms as f64 / 1000.0);
                    let pos = self.playback.time_pos;
                    if pos >= 10.0 && (dur <= 0.0 || pos >= (dur * 0.5).min(240.0)) {
                        self.scrobbled_current = true;
                        let tid = track.id.clone();
                        let artist = track.artist_name.clone();
                        let title = track.title.clone();
                        let sk = self.tui_config.lastfm_session_key.clone();
                        let client = self.client.clone();
                        tokio::spawn(async move {
                            let _ = client.scrobble_track(&tid).await;
                            if let Some(key) = sk {
                                let ts = chrono::Utc::now().timestamp();
                                let _ = client.lastfm_scrobble(&key, &artist, &title, ts).await;
                            }
                        });
                    }
                }
            }

            // Periodic podcast progress save every 10s
            if let Some(NowPlayingItem::Podcast { episode, .. }) = &self.now_playing {
                if self.last_podcast_save.elapsed() >= Duration::from_secs(10) && self.playback.time_pos > 1.0 {
                    self.last_podcast_save = Instant::now();
                    let ep_id = episode.id_string();
                    let pos_ms = (self.playback.time_pos * 1000.0) as i64;
                    let completed = self.playback.duration > 10.0
                        && (self.playback.duration - self.playback.time_pos) < 15.0;
                    self.podcast_progress.insert(
                        ep_id.clone(),
                        PodcastProgress {
                            episode_id: ep_id.clone(),
                            position_ms: pos_ms,
                            completed,
                            ..Default::default()
                        },
                    );
                    let client = self.client.clone();
                    tokio::spawn(async move {
                        let _ = client.podcast_save_progress(&ep_id, pos_ms, completed).await;
                    });
                }
            }

            // Auto-advance queue when mpv becomes idle after playing
            if self.playback.idle_active
                && self
                    .play_started_at
                    .map(|t| t.elapsed() > Duration::from_secs(2))
                    .unwrap_or(false)
            {
                if matches!(self.now_playing, Some(NowPlayingItem::Track(_))) {
                    if let Some(idx) = self.queue_idx {
                        if idx + 1 < self.queue.len() {
                            self.play_next().await;
                        } else {
                            self.now_playing = None;
                        }
                    }
                }
            }
        }
    }

    pub async fn submit_login(&mut self) {
        let Modal::Login {
            url,
            username,
            password,
            invite_code,
            is_register,
            ..
        } = &self.modal
        else {
            return;
        };
        let url = url.trim().trim_end_matches('/').to_string();
        let username = username.trim().to_string();
        let password = password.clone();
        let invite_code = invite_code.trim().to_string();
        let is_register = *is_register;

        match SupernovaClient::new(url.clone(), None) {
            Ok(mut c) => {
                let res = if is_register {
                    c.register(&username, &password, &invite_code).await
                } else {
                    c.login(&username, &password).await
                };
                match res {
                    Ok(auth) => {
                        self.tui_config.url = Some(url.clone());
                        self.tui_config.token = Some(auth.token.clone());
                        let _ = save_config(&self.tui_config);
                        self.client = c;
                        self.user = Some(auth.user);
                        self.modal = Modal::None;
                        self.status_msg = format!("Logged in to {url}");
                        self.refresh_initial().await;
                    }
                    Err(e) => {
                        if let Modal::Login { error, .. } = &mut self.modal {
                            *error = Some(e.to_string());
                        }
                    }
                }
            }
            Err(e) => {
                if let Modal::Login { error, .. } = &mut self.modal {
                    *error = Some(e.to_string());
                }
            }
        }
    }

    pub async fn logout_and_prompt(&mut self) {
        let _ = self.client.logout().await;
        self.tui_config.token = None;
        let _ = save_config(&self.tui_config);
        self.user = None;
        self.modal = Modal::Login {
            url: self.client.base_url.clone(),
            username: String::new(),
            password: String::new(),
            invite_code: String::new(),
            is_register: false,
            field: 1,
            error: None,
        };
    }

    pub fn save_cfg(&self) {
        let _ = save_config(&self.tui_config);
    }
}
