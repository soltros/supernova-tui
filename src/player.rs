use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};

pub struct MpvPlayer {
    child: Option<Child>,
    socket_path: PathBuf,
    req_id: u64,
    pub available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PlaybackStatus {
    pub time_pos: f64,
    pub duration: f64,
    pub paused: bool,
    pub idle_active: bool,
    pub volume: f64,
}

impl MpvPlayer {
    pub fn new() -> Self {
        let pid = std::process::id();
        let socket_path = std::env::temp_dir().join(format!("supernova-tui-mpv-{pid}.sock"));
        Self {
            child: None,
            socket_path,
            req_id: 1,
            available: false,
        }
    }

    pub async fn ensure_started(&mut self) -> Result<()> {
        if let Some(child) = &mut self.child {
            if let Ok(None) = child.try_wait() {
                self.available = true;
                return Ok(());
            }
        }
        let _ = std::fs::remove_file(&self.socket_path);
        let child = Command::new("mpv")
            .arg("--no-video")
            .arg("--idle=yes")
            .arg("--really-quiet")
            .arg("--no-terminal")
            .arg(format!("--input-ipc-server={}", self.socket_path.display()))
            .spawn()
            .map_err(|e| anyhow!("Failed to start mpv: {e}"))?;
        self.child = Some(child);

        for _ in 0..30 {
            if self.socket_path.exists() {
                if UnixStream::connect(&self.socket_path).await.is_ok() {
                    self.available = true;
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(anyhow!("Timed out waiting for mpv IPC socket"))
    }

    async fn send_cmd(&mut self, cmd: Value) -> Result<Value> {
        self.ensure_started().await?;
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let id = self.req_id;
        self.req_id += 1;
        let payload = json!({
            "command": cmd,
            "request_id": id
        });
        let mut line = serde_json::to_string(&payload)?;
        line.push('\n');
        stream.write_all(line.as_bytes()).await?;

        let mut reader = BufReader::new(stream);
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = tokio::time::timeout(Duration::from_millis(400), reader.read_line(&mut buf))
                .await
                .map_err(|_| anyhow!("mpv IPC timeout"))??;
            if n == 0 {
                return Err(anyhow!("mpv IPC EOF"));
            }
            if let Ok(v) = serde_json::from_str::<Value>(&buf) {
                if v.get("request_id").and_then(|x| x.as_u64()) == Some(id) {
                    return Ok(v);
                }
            }
        }
    }

    pub async fn play_url(&mut self, url: &str, start_secs: Option<f64>) -> Result<()> {
        self.send_cmd(json!(["loadfile", url, "replace"])).await?;
        self.send_cmd(json!(["set_property", "pause", false])).await?;
        if let Some(secs) = start_secs {
            if secs > 1.0 {
                tokio::time::sleep(Duration::from_millis(120)).await;
                let _ = self.seek_abs(secs).await;
            }
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if self.child.is_some() {
            let _ = self.send_cmd(json!(["stop"])).await;
        }
        Ok(())
    }

    pub async fn toggle_pause(&mut self) -> Result<bool> {
        let cur = self.get_bool("pause").await.unwrap_or(false);
        let next = !cur;
        self.send_cmd(json!(["set_property", "pause", next])).await?;
        Ok(next)
    }

    pub async fn seek_rel(&mut self, delta_secs: f64) -> Result<()> {
        self.send_cmd(json!(["seek", delta_secs, "relative"])).await?;
        Ok(())
    }

    pub async fn seek_abs(&mut self, secs: f64) -> Result<()> {
        self.send_cmd(json!(["seek", secs.max(0.0), "absolute"])).await?;
        Ok(())
    }

    pub async fn set_volume(&mut self, vol: u8) -> Result<()> {
        self.send_cmd(json!(["set_property", "volume", vol as f64])).await?;
        Ok(())
    }

    async fn get_f64(&mut self, prop: &str) -> Option<f64> {
        let v = self.send_cmd(json!(["get_property", prop])).await.ok()?;
        v.get("data")?.as_f64()
    }

    async fn get_bool(&mut self, prop: &str) -> Option<bool> {
        let v = self.send_cmd(json!(["get_property", prop])).await.ok()?;
        v.get("data")?.as_bool()
    }

    pub async fn poll_status(&mut self) -> PlaybackStatus {
        if self.child.is_none() {
            return PlaybackStatus::default();
        }
        PlaybackStatus {
            time_pos: self.get_f64("time-pos").await.unwrap_or(0.0),
            duration: self.get_f64("duration").await.unwrap_or(0.0),
            paused: self.get_bool("pause").await.unwrap_or(false),
            idle_active: self.get_bool("idle-active").await.unwrap_or(true),
            volume: self.get_f64("volume").await.unwrap_or(100.0),
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
