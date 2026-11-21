use anyhow::{anyhow, Result};
use reqwest::{Client, Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use url::Url;

use crate::models::*;

#[derive(Clone)]
pub struct SupernovaClient {
    http: Client,
    pub base_url: String,
    pub token: Option<String>,
}

impl SupernovaClient {
    pub fn new(base_url: impl Into<String>, token: Option<String>) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            base_url,
            token,
        })
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut rb = self.http.request(method, url);
        if let Some(tok) = &self.token {
            rb = rb.bearer_auth(tok);
        }
        rb
    }

    async fn send_json<T: DeserializeOwned>(&self, rb: RequestBuilder) -> Result<T> {
        let resp = rb.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("HTTP {}: {}", status.as_u16(), text.trim()));
        }
        let body = resp.text().await?;
        serde_json::from_str(&body).map_err(|e| anyhow!("JSON decode error: {e}"))
    }

    async fn send_empty(&self, rb: RequestBuilder) -> Result<()> {
        let resp = rb.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("HTTP {}: {}", status.as_u16(), text.trim()));
        }
        Ok(())
    }

    // --- Health & Auth ---

    pub async fn health(&self) -> Result<serde_json::Value> {
        self.send_json(self.req(Method::GET, "/api/health")).await
    }

    pub async fn login(&mut self, username: &str, password: &str) -> Result<AuthResponse> {
        let res: AuthResponse = self
            .send_json(
                self.req(Method::POST, "/api/auth/login")
                    .json(&json!({ "username": username, "password": password })),
            )
            .await?;
        self.token = Some(res.token.clone());
        Ok(res)
    }

    pub async fn register(
        &mut self,
        username: &str,
        password: &str,
        invite_code: &str,
    ) -> Result<AuthResponse> {
        let res: AuthResponse = self
            .send_json(self.req(Method::POST, "/api/auth/register").json(&json!({
                "username": username,
                "password": password,
                "invite_code": invite_code
            })))
            .await?;
        self.token = Some(res.token.clone());
        Ok(res)
    }

    pub async fn logout(&mut self) -> Result<()> {
        let res = self.send_empty(self.req(Method::POST, "/api/auth/logout")).await;
        self.token = None;
        res
    }

    pub async fn me(&self) -> Result<User> {
        self.send_json(self.req(Method::GET, "/api/auth/me")).await
    }

    pub async fn change_password(&self, current_password: &str, new_password: &str) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/auth/change-password")
                .json(&json!({
                    "current_password": current_password,
                    "new_password": new_password
                })),
        )
        .await
    }

    pub async fn create_media_ticket(&self, scope: &str, resource: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct TicketResp {
            ticket: String,
        }
        let resp: TicketResp = self
            .send_json(
                self.req(Method::POST, "/api/media-ticket")
                    .json(&json!({ "scope": scope, "resource": resource })),
            )
            .await?;
        Ok(resp.ticket)
    }

    pub async fn stream_url(
        &self,
        track_id: &str,
        format: Option<&str>,
        bitrate: Option<u32>,
    ) -> Result<String> {
        let ticket = self.create_media_ticket("stream", track_id).await?;
        let mut url = Url::parse(&format!("{}/api/stream/{}", self.base_url, track_id))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("ticket", &ticket);
            if let Some(fmt) = format {
                if !fmt.is_empty() && fmt != "raw" {
                    pairs.append_pair("format", fmt);
                }
            }
            if let Some(br) = bitrate {
                if br > 0 {
                    pairs.append_pair("bitrate", &br.to_string());
                }
            }
        }
        Ok(url.to_string())
    }

    // --- Dashboard, Discovery, Library ---

    pub async fn dashboard(&self) -> Result<DashboardData> {
        self.send_json(self.req(Method::GET, "/api/dashboard")).await
    }

    pub async fn discovery(&self) -> Result<DiscoveryData> {
        self.send_json(self.req(Method::GET, "/api/discovery")).await
    }

    pub async fn artists(&self, limit: usize, offset: usize, letter: Option<&str>) -> Result<Vec<Artist>> {
        let mut url = format!("/api/artists?limit={limit}&offset={offset}");
        if let Some(l) = letter {
            if !l.is_empty() {
                url.push_str(&format!("&letter={}", urlencoding(l)));
            }
        }
        self.send_json(self.req(Method::GET, &url)).await
    }

    pub async fn artist_by_id(&self, id: &str) -> Result<Artist> {
        self.send_json(self.req(Method::GET, &format!("/api/artists/{id}")))
            .await
    }

    pub async fn albums(&self, limit: usize, offset: usize, artist_id: Option<&str>) -> Result<Vec<Album>> {
        let mut url = format!("/api/albums?limit={limit}&offset={offset}");
        if let Some(aid) = artist_id {
            if !aid.is_empty() {
                url.push_str(&format!("&artist_id={}", urlencoding(aid)));
            }
        }
        self.send_json(self.req(Method::GET, &url)).await
    }

    pub async fn album_by_id(&self, id: &str) -> Result<Album> {
        self.send_json(self.req(Method::GET, &format!("/api/albums/{id}")))
            .await
    }

    pub async fn tracks(
        &self,
        album_id: Option<&str>,
        artist_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Track>> {
        let mut url = format!("/api/tracks?limit={limit}&offset={offset}");
        if let Some(al) = album_id {
            if !al.is_empty() {
                url.push_str(&format!("&album_id={}", urlencoding(al)));
            }
        }
        if let Some(ar) = artist_id {
            if !ar.is_empty() {
                url.push_str(&format!("&artist_id={}", urlencoding(ar)));
            }
        }
        self.send_json(self.req(Method::GET, &url)).await
    }

    pub async fn search(&self, query: &str) -> Result<SearchResults> {
        self.send_json(self.req(Method::GET, &format!("/api/search?q={}", urlencoding(query))))
            .await
    }

    // --- Scrobbles ---

    pub async fn scrobble_track(&self, track_id: &str) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/scrobbles")
                .json(&json!({ "track_id": track_id })),
        )
        .await
    }

    pub async fn recent_scrobbles(&self) -> Result<Vec<Track>> {
        self.send_json(self.req(Method::GET, "/api/scrobbles/recent"))
            .await
    }

    // --- Hearts ---

    pub async fn hearts(&self) -> Result<Vec<Heart>> {
        self.send_json(self.req(Method::GET, "/api/hearts")).await
    }

    pub async fn heart_details(&self) -> Result<HeartDetails> {
        self.send_json(self.req(Method::GET, "/api/hearts/details"))
            .await
    }

    pub async fn add_heart(
        &self,
        entity_type: &str,
        entity_id: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        self.send_empty(self.req(Method::POST, "/api/hearts").json(&json!({
            "entity_type": entity_type,
            "entity_id": entity_id,
            "metadata": metadata
        })))
        .await
    }

    pub async fn remove_heart(&self, entity_type: &str, entity_id: &str) -> Result<()> {
        self.send_empty(self.req(
            Method::DELETE,
            &format!(
                "/api/hearts?entity_type={}&entity_id={}",
                urlencoding(entity_type),
                urlencoding(entity_id)
            ),
        ))
        .await
    }

    pub async fn export_hearts(&self) -> Result<String> {
        let resp = self.req(Method::GET, "/api/hearts/export").send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        Ok(resp.text().await?)
    }

    pub async fn import_hearts(&self, json_body: String) -> Result<serde_json::Value> {
        self.send_json(
            self.req(Method::POST, "/api/hearts/import")
                .header("Content-Type", "application/json")
                .body(json_body),
        )
        .await
    }

    // --- Playlists ---

    pub async fn playlists(&self) -> Result<Vec<Playlist>> {
        self.send_json(self.req(Method::GET, "/api/playlists")).await
    }

    pub async fn create_playlist(&self, name: &str) -> Result<Playlist> {
        self.send_json(
            self.req(Method::POST, "/api/playlists")
                .json(&json!({ "name": name })),
        )
        .await
    }

    pub async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.send_empty(self.req(Method::DELETE, &format!("/api/playlists/{id}")))
            .await
    }

    pub async fn playlist_tracks(&self, id: &str) -> Result<Vec<Track>> {
        self.send_json(self.req(Method::GET, &format!("/api/playlists/{id}/tracks")))
            .await
    }

    pub async fn add_track_to_playlist(&self, playlist_id: &str, track_id: &str) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, &format!("/api/playlists/{playlist_id}/tracks"))
                .json(&json!({ "track_id": track_id })),
        )
        .await
    }

    pub async fn remove_track_from_playlist(&self, playlist_id: &str, track_id: &str) -> Result<()> {
        self.send_empty(self.req(
            Method::DELETE,
            &format!("/api/playlists/{playlist_id}/tracks/{track_id}"),
        ))
        .await
    }

    pub async fn export_playlists(&self) -> Result<String> {
        let resp = self.req(Method::GET, "/api/playlists/export").send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        Ok(resp.text().await?)
    }

    // --- Downloads ---

    pub async fn download_track_to(&self, track_id: &str, dest: &std::path::Path) -> Result<()> {
        let ticket = self.create_media_ticket("download-track", track_id).await?;
        let url = format!(
            "{}/api/download/track/{}?ticket={}",
            self.base_url,
            track_id,
            urlencoding(&ticket)
        );
        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        let bytes = resp.bytes().await?;
        tokio::fs::write(dest, &bytes).await?;
        Ok(())
    }

    // --- Admin / Scan / Settings ---

    pub async fn scan_library(&self) -> Result<()> {
        self.send_empty(self.req(Method::POST, "/api/scan")).await
    }

    pub async fn scan_progress(&self) -> Result<ScanProgress> {
        self.send_json(self.req(Method::GET, "/api/scan/progress"))
            .await
    }

    pub async fn reset_artists(&self) -> Result<()> {
        self.send_empty(self.req(Method::POST, "/api/settings/reset-artists"))
            .await
    }

    // --- Plugins ---

    pub async fn plugins(&self) -> Result<Vec<PluginInfo>> {
        self.send_json(self.req(Method::GET, "/api/plugins")).await
    }

    pub async fn run_autotagger(&self) -> Result<serde_json::Value> {
        self.send_json(self.req(Method::POST, "/api/plugins/autotagger/run"))
            .await
    }

    pub async fn preview_artist_merger(&self) -> Result<MergePreviewResponse<ArtistMergeCandidate>> {
        self.send_json(self.req(Method::GET, "/api/plugins/artistmerger/preview"))
            .await
    }

    pub async fn preview_album_merger(&self) -> Result<MergePreviewResponse<AlbumMergeCandidate>> {
        self.send_json(self.req(Method::GET, "/api/plugins/albummerger/preview"))
            .await
    }

    pub async fn preview_deduper(&self) -> Result<MergePreviewResponse<DedupeCandidate>> {
        self.send_json(self.req(Method::GET, "/api/plugins/deduper/preview"))
            .await
    }

    // LRCLib plugin
    pub async fn get_lyrics(
        &self,
        artist: &str,
        track: &str,
        album: &str,
        duration_sec: i64,
    ) -> Result<Option<LyricsResponse>> {
        let url = format!(
            "/api/plugins/lrclib/lyrics?artist_name={}&track_name={}&album_name={}&duration={}",
            urlencoding(artist),
            urlencoding(track),
            urlencoding(album),
            duration_sec.max(0)
        );
        let resp = self.req(Method::GET, &url).send().await?;
        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("HTTP {}: {}", status.as_u16(), text.trim()));
        }
        Ok(Some(resp.json().await?))
    }

    // Last.fm plugin
    pub async fn lastfm_auth_url(&self, cb: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Resp {
            url: String,
        }
        let r: Resp = self
            .send_json(self.req(
                Method::GET,
                &format!("/api/plugins/lastfm/auth-url?cb={}", urlencoding(cb)),
            ))
            .await?;
        Ok(r.url)
    }

    pub async fn lastfm_exchange_token(&self, token: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Resp {
            session_key: String,
        }
        let r: Resp = self
            .send_json(
                self.req(Method::POST, "/api/plugins/lastfm/session")
                    .json(&json!({ "token": token })),
            )
            .await?;
        Ok(r.session_key)
    }

    pub async fn lastfm_now_playing(
        &self,
        session_key: &str,
        artist: &str,
        track: &str,
    ) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/plugins/lastfm/nowplaying")
                .json(&json!({
                    "session_key": session_key,
                    "artist": artist,
                    "track": track
                })),
        )
        .await
    }

    pub async fn lastfm_scrobble(
        &self,
        session_key: &str,
        artist: &str,
        track: &str,
        timestamp: i64,
    ) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/plugins/lastfm/scrobble")
                .json(&json!({
                    "session_key": session_key,
                    "artist": artist,
                    "track": track,
                    "timestamp": timestamp
                })),
        )
        .await
    }

    // Podcasts plugin
    pub async fn podcast_search(&self, query: &str) -> Result<Vec<PodcastFeed>> {
        let val: serde_json::Value = self
            .send_json(self.req(
                Method::GET,
                &format!("/api/plugins/podcasts/search?q={}", urlencoding(query)),
            ))
            .await?;
        if val.is_null() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_value(val)?)
    }

    pub async fn podcast_episodes(&self, feed_id: &str) -> Result<Vec<PodcastEpisode>> {
        let val: serde_json::Value = self
            .send_json(self.req(
                Method::GET,
                &format!("/api/plugins/podcasts/episodes?id={}", urlencoding(feed_id)),
            ))
            .await?;
        if val.is_null() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_value(val)?)
    }

    pub async fn podcast_subscriptions(&self) -> Result<Vec<PodcastSubscription>> {
        self.send_json(self.req(Method::GET, "/api/plugins/podcasts/subscriptions"))
            .await
    }

    pub async fn podcast_subscribe(
        &self,
        feed_id: &str,
        feed_url: &str,
        title: &str,
        image_url: &str,
    ) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/plugins/podcasts/subscriptions")
                .json(&json!({
                    "feed_id": feed_id,
                    "feed_url": feed_url,
                    "title": title,
                    "image_url": image_url
                })),
        )
        .await
    }

    pub async fn podcast_unsubscribe(&self, feed_id: &str) -> Result<()> {
        self.send_empty(self.req(
            Method::DELETE,
            &format!("/api/plugins/podcasts/subscriptions?feed_id={}", urlencoding(feed_id)),
        ))
        .await
    }

    pub async fn podcast_save_progress(
        &self,
        episode_id: &str,
        position_ms: i64,
        completed: bool,
    ) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/plugins/podcasts/progress")
                .json(&json!({
                    "episode_id": episode_id,
                    "position_ms": position_ms,
                    "completed": completed
                })),
        )
        .await
    }

    pub async fn podcast_progress_batch(
        &self,
        episode_ids: &[String],
    ) -> Result<PodcastProgressMap> {
        if episode_ids.is_empty() {
            return Ok(PodcastProgressMap::new());
        }
        self.send_json(
            self.req(Method::POST, "/api/plugins/podcasts/progress/batch")
                .json(&json!({ "episode_ids": episode_ids })),
        )
        .await
    }

    pub async fn podcast_export_opml(&self) -> Result<String> {
        let resp = self
            .req(Method::GET, "/api/plugins/podcasts/opml/export")
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        Ok(resp.text().await?)
    }

    pub async fn podcast_import_opml(&self, filename: &str, opml_bytes: Vec<u8>) -> Result<serde_json::Value> {
        let boundary = "----SupernovaTuiOpmlBoundary7MA4YWxkTrZu0gW";
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/xml\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(&opml_bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        self.send_json(
            self.req(Method::POST, "/api/plugins/podcasts/opml/import")
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(body),
        )
        .await
    }

    // RadioBrowser plugin
    pub async fn radio_search(
        &self,
        query: &str,
        country: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<RadioStation>> {
        let url = format!(
            "/api/plugins/radio/search?q={}&country={}&limit={limit}&offset={offset}",
            urlencoding(query),
            urlencoding(country)
        );
        self.send_json(self.req(Method::GET, &url)).await
    }

    pub async fn radio_subscriptions(&self) -> Result<Vec<RadioSubscription>> {
        self.send_json(self.req(Method::GET, "/api/plugins/radio/subscriptions"))
            .await
    }

    pub async fn radio_subscribe(
        &self,
        station_id: &str,
        url: &str,
        name: &str,
        favicon: &str,
    ) -> Result<()> {
        self.send_empty(
            self.req(Method::POST, "/api/plugins/radio/subscriptions")
                .json(&json!({
                    "station_id": station_id,
                    "url": url,
                    "name": name,
                    "favicon": favicon
                })),
        )
        .await
    }

    pub async fn radio_unsubscribe(&self, station_id: &str) -> Result<()> {
        self.send_empty(self.req(
            Method::DELETE,
            &format!(
                "/api/plugins/radio/subscriptions?station_id={}",
                urlencoding(station_id)
            ),
        ))
        .await
    }

    // Subsonic plugin ping check
    pub async fn subsonic_ping_status(&self) -> Result<String> {
        let resp = self.req(Method::GET, "/rest/ping?f=json").send().await?;
        Ok(format!("HTTP {}", resp.status().as_u16()))
    }
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
