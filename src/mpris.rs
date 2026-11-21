use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use zbus::zvariant::{ObjectPath, Value};
use zbus::{interface, object_server::SignalContext, Connection};

#[derive(Debug, Clone)]
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Seek(f64),
    SetPosition(f64),
    SetVolume(u8),
    Quit,
}

#[derive(Debug, Clone, Default)]
pub struct MprisState {
    pub playback_status: String, // "Playing", "Paused", "Stopped"
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: String,
    pub length_us: i64,
    pub position_us: i64,
    pub volume: f64, // 0.0..=1.0
}

struct MediaPlayer2 {
    tx: mpsc::UnboundedSender<MprisCommand>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl MediaPlayer2 {
    fn raise(&self) {}

    fn quit(&self) {
        let _ = self.tx.send(MprisCommand::Quit);
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Supernova TUI"
    }

    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "supernova-tui"
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["http".to_string(), "https".to_string()]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        vec![
            "audio/mpeg".to_string(),
            "audio/flac".to_string(),
            "audio/ogg".to_string(),
            "audio/mp4".to_string(),
            "audio/aac".to_string(),
        ]
    }
}

struct MediaPlayer2Player {
    state: Arc<RwLock<MprisState>>,
    tx: mpsc::UnboundedSender<MprisCommand>,
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MediaPlayer2Player {
    fn next(&self) {
        let _ = self.tx.send(MprisCommand::Next);
    }

    fn previous(&self) {
        let _ = self.tx.send(MprisCommand::Previous);
    }

    fn pause(&self) {
        let _ = self.tx.send(MprisCommand::Pause);
    }

    fn play_pause(&self) {
        let _ = self.tx.send(MprisCommand::PlayPause);
    }

    fn stop(&self) {
        let _ = self.tx.send(MprisCommand::Stop);
    }

    fn play(&self) {
        let _ = self.tx.send(MprisCommand::Play);
    }

    fn seek(&self, offset_us: i64) {
        let _ = self.tx.send(MprisCommand::Seek(offset_us as f64 / 1_000_000.0));
    }

    fn set_position(&self, _track_id: ObjectPath<'_>, position_us: i64) {
        let _ = self
            .tx
            .send(MprisCommand::SetPosition(position_us as f64 / 1_000_000.0));
    }

    fn open_uri(&self, _uri: String) {}

    #[zbus(property)]
    async fn playback_status(&self) -> String {
        let s = self.state.read().await;
        if s.playback_status.is_empty() {
            "Stopped".to_string()
        } else {
            s.playback_status.clone()
        }
    }

    #[zbus(property)]
    async fn rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<String, Value<'static>> {
        let s = self.state.read().await;
        let mut map = HashMap::new();
        if s.title.is_empty() {
            return map;
        }
        let clean_id: String = s
            .track_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let path_str = if clean_id.is_empty() {
            "/org/mpris/MediaPlayer2/Track/0".to_string()
        } else {
            format!("/org/mpris/MediaPlayer2/Track/{clean_id}")
        };
        if let Ok(obj) = ObjectPath::try_from(path_str) {
            map.insert("mpris:trackid".to_string(), Value::from(obj));
        }
        map.insert("xesam:title".to_string(), Value::from(s.title.clone()));
        if !s.artist.is_empty() {
            map.insert(
                "xesam:artist".to_string(),
                Value::from(vec![s.artist.clone()]),
            );
        }
        if !s.album.is_empty() {
            map.insert("xesam:album".to_string(), Value::from(s.album.clone()));
        }
        if !s.art_url.is_empty() {
            map.insert("mpris:artUrl".to_string(), Value::from(s.art_url.clone()));
        }
        if s.length_us > 0 {
            map.insert("mpris:length".to_string(), Value::from(s.length_us));
        }
        map
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        self.state.read().await.volume
    }

    #[zbus(property)]
    async fn set_volume(&self, vol: f64) {
        let v = (vol.clamp(0.0, 1.0) * 100.0).round() as u8;
        let _ = self.tx.send(MprisCommand::SetVolume(v));
    }

    #[zbus(property)]
    async fn position(&self) -> i64 {
        self.state.read().await.position_us
    }

    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

pub struct MprisHandle {
    state: Arc<RwLock<MprisState>>,
    conn: Option<Connection>,
}

impl MprisHandle {
    pub async fn start(tx: mpsc::UnboundedSender<MprisCommand>) -> Self {
        let state = Arc::new(RwLock::new(MprisState {
            playback_status: "Stopped".to_string(),
            volume: 1.0,
            ..Default::default()
        }));

        let root = MediaPlayer2 { tx: tx.clone() };
        let player = MediaPlayer2Player {
            state: state.clone(),
            tx,
        };

        let conn = match Connection::session().await {
            Ok(c) => {
                let ok_root = c.object_server().at("/org/mpris/MediaPlayer2", root).await.is_ok();
                let ok_player = c
                    .object_server()
                    .at("/org/mpris/MediaPlayer2", player)
                    .await
                    .is_ok();
                let bus_name = format!("org.mpris.MediaPlayer2.supernova_tui.instance{}", std::process::id());
                let ok_name = c.request_name(bus_name.as_str()).await.is_ok()
                    || c.request_name("org.mpris.MediaPlayer2.supernova_tui").await.is_ok();
                if ok_root && ok_player && ok_name {
                    Some(c)
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        Self { state, conn }
    }

    pub async fn update(&self, new_state: MprisState) {
        let (changed_status, changed_meta, changed_vol) = {
            let mut cur = self.state.write().await;
            let cs = cur.playback_status != new_state.playback_status;
            let cm = cur.track_id != new_state.track_id || cur.title != new_state.title;
            let cv = (cur.volume - new_state.volume).abs() > 0.005;
            *cur = new_state;
            (cs, cm, cv)
        };

        if let Some(conn) = &self.conn {
            if changed_status || changed_meta || changed_vol {
                if let Ok(iface_ref) = conn
                    .object_server()
                    .interface::<_, MediaPlayer2Player>("/org/mpris/MediaPlayer2")
                    .await
                {
                    let ctxt = SignalContext::new(conn, "/org/mpris/MediaPlayer2").ok();
                    if let Some(ctxt) = ctxt {
                        let iface = iface_ref.get().await;
                        if changed_status {
                            let _ = iface.playback_status_changed(&ctxt).await;
                        }
                        if changed_meta {
                            let _ = iface.metadata_changed(&ctxt).await;
                        }
                        if changed_vol {
                            let _ = iface.volume_changed(&ctxt).await;
                        }
                    }
                }
            }
        }
    }
}
