use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    pub is_admin: bool,
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub musicbrainz_id: String,
    #[serde(default)]
    pub image_url: String,
    #[serde(default)]
    pub bio: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub release_year: i32,
    #[serde(default)]
    pub musicbrainz_id: String,
    #[serde(default)]
    pub cover_art_path: String,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub artist_id: String,
    #[serde(default)]
    pub artist_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    #[serde(default)]
    pub album_id: String,
    pub title: String,
    #[serde(default)]
    pub track_number: i32,
    #[serde(default)]
    pub disc_number: i32,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub bitrate: i32,
    #[serde(default)]
    pub artist_id: String,
    #[serde(default)]
    pub artist_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResults {
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub albums: Vec<Album>,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardData {
    #[serde(default)]
    pub recently_added_albums: Vec<Album>,
    #[serde(default)]
    pub recently_played_tracks: Vec<Track>,
    #[serde(default)]
    pub favorite_tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRelease {
    #[serde(rename = "collectionName", default)]
    pub collection_name: String,
    #[serde(rename = "artistName", default)]
    pub artist_name: String,
    #[serde(rename = "artworkUrl100", default)]
    pub artwork_url_100: String,
    #[serde(rename = "releaseDate", default)]
    pub release_date: String,
    #[serde(rename = "collectionViewUrl", default)]
    pub collection_view_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarArtist {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "basedOn", default)]
    pub based_on: String,
    #[serde(default)]
    pub image: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryData {
    #[serde(default)]
    pub release_radar: Vec<ExternalRelease>,
    #[serde(default)]
    pub similar_artists: Vec<SimilarArtist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heart {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartDetails {
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub albums: Vec<Album>,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub playlists: Vec<Playlist>,
    #[serde(default)]
    pub radio: Vec<serde_json::Value>,
    #[serde(default)]
    pub podcasts: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    pub name: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanProgress {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub files_scanned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricsResponse {
    #[serde(default)]
    pub id: i64,
    #[serde(rename = "trackName", default)]
    pub track_name: String,
    #[serde(rename = "artistName", default)]
    pub artist_name: String,
    #[serde(rename = "albumName", default)]
    pub album_name: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(rename = "plainLyrics", default)]
    pub plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics", default)]
    pub synced_lyrics: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LrcLine {
    pub time_sec: f64,
    pub text: String,
}

pub fn parse_lrc(synced: &str) -> Vec<LrcLine> {
    let mut lines = Vec::new();
    for raw in synced.lines() {
        let raw = raw.trim();
        if !raw.starts_with('[') {
            continue;
        }
        if let Some(close) = raw.find(']') {
            let ts = &raw[1..close];
            let text = raw[close + 1..].trim().to_string();
            if let Some((min_s, sec_s)) = ts.split_once(':') {
                if let (Ok(min), Ok(sec)) = (min_s.parse::<f64>(), sec_s.parse::<f64>()) {
                    lines.push(LrcLine {
                        time_sec: min * 60.0 + sec,
                        text,
                    });
                }
            }
        }
    }
    lines.sort_by(|a, b| a.time_sec.partial_cmp(&b.time_sec).unwrap_or(std::cmp::Ordering::Equal));
    lines
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastFeed {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub artwork: String,
}

impl PodcastFeed {
    pub fn id_string(&self) -> String {
        match &self.id {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }
    pub fn best_image(&self) -> String {
        if !self.artwork.is_empty() {
            self.artwork.clone()
        } else {
            self.image.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastEpisode {
    #[serde(default)]
    pub id: serde_json::Value,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "enclosureUrl", default)]
    pub enclosure_url: String,
    #[serde(default)]
    pub duration: i64,
    #[serde(rename = "datePublished", default)]
    pub date_published: i64,
    #[serde(rename = "datePublishedPretty", default)]
    pub date_published_pretty: String,
}

impl PodcastEpisode {
    pub fn id_string(&self) -> String {
        match &self.id {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastSubscription {
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    pub feed_id: String,
    #[serde(default)]
    pub feed_url: String,
    pub title: String,
    #[serde(default)]
    pub image_url: String,
    #[serde(default)]
    pub subscribed_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodcastProgress {
    #[serde(default)]
    pub user_id: String,
    pub episode_id: String,
    pub position_ms: i64,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub updated_at: String,
}

pub type PodcastProgressMap = HashMap<String, PodcastProgress>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioStation {
    #[serde(default)]
    pub stationuuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub url_resolved: String,
    #[serde(default)]
    pub favicon: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub bitrate: i32,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub clickcount: i64,
}

impl RadioStation {
    pub fn stream_url(&self) -> &str {
        if !self.url_resolved.is_empty() {
            &self.url_resolved
        } else {
            &self.url
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioSubscription {
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    pub station_id: String,
    pub url: String,
    pub name: String,
    #[serde(default)]
    pub favicon: String,
    #[serde(default)]
    pub subscribed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergePreviewResponse<T> {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub warning: String,
    #[serde(default)]
    pub candidates: Vec<Vec<T>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ArtistMergeCandidate {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "MBID", default)]
    pub mbid: String,
    #[serde(default)]
    pub albums: i64,
    #[serde(default)]
    pub tracks: i64,
    #[serde(default)]
    pub hearts: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AlbumMergeCandidate {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "MBID", default)]
    pub mbid: String,
    #[serde(rename = "ArtistID", default)]
    pub artist_id: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub year: i64,
    #[serde(default)]
    pub tracks: i64,
    #[serde(default)]
    pub hearts: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DedupeCandidate {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "AlbumID", default)]
    pub album_id: String,
    #[serde(default)]
    pub album: String,
    #[serde(rename = "ArtistID", default)]
    pub artist_id: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub disc: i64,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub bitrate: i64,
    #[serde(default)]
    pub playlist_entries: i64,
    #[serde(default)]
    pub hearts: i64,
    #[serde(default)]
    pub scrobbles: i64,
}

pub fn format_ms(ms: i64) -> String {
    let total_sec = (ms / 1000).max(0);
    let hours = total_sec / 3600;
    let mins = (total_sec % 3600) / 60;
    let secs = total_sec % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}

pub fn format_secs(sec: f64) -> String {
    format_ms((sec * 1000.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc_and_format() {
        let lrc = "[00:12.50] First line\n[01:05.00] Second line\n[invalid] skip";
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.len(), 2);
        assert!((parsed[0].time_sec - 12.5).abs() < 0.01);
        assert_eq!(parsed[0].text, "First line");
        assert!((parsed[1].time_sec - 65.0).abs() < 0.01);
        assert_eq!(format_ms(65_000), "1:05");
        assert_eq!(format_ms(3_665_000), "1:01:05");
    }

    #[test]
    fn test_merge_preview_deserialize() {
        let json = r#"{
            "mode": "read-only",
            "warning": "test warning",
            "candidates": [[
                {"ID": "1", "Name": "The Beatles", "MBID": "", "Albums": 12, "Tracks": 180, "Hearts": 3}
            ]]
        }"#;
        let parsed: MergePreviewResponse<ArtistMergeCandidate> = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.mode, "read-only");
        assert_eq!(parsed.candidates.len(), 1);
        assert_eq!(parsed.candidates[0][0].name, "The Beatles");
    }
}
