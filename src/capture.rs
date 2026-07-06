//! Headless-browser capture sidecar for agent vision (`GET /observe/image`).
//!
//! Renders agent point-of-view frames through the exact same pipeline a human
//! spectator sees: the world's own Three.js renderer bundle running in the
//! embedded runtime frontend. A single headless Chromium process is spawned
//! lazily on the first capture request and kept alive; it loads the local
//! spectator page (`/?viewport=1`) and is driven over the Chrome DevTools
//! Protocol: set a camera pose via the page's `window.__clawbloxCapture` hook,
//! wait for the renderer to draw, then `Page.captureScreenshot`.
//!
//! The browser is launched with `DISPLAY`/`WAYLAND_DISPLAY` scrubbed from its
//! environment so Chromium falls back to SwiftShader software WebGL. This works
//! on GPU-less servers, WSL2, and CI without an X server, and keeps rendering
//! deterministic across hosts.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

const BROWSER_START_TIMEOUT: Duration = Duration::from_secs(30);
const PAGE_READY_TIMEOUT: Duration = Duration::from_secs(60);
const CDP_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const FRAME_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// First-person attach config sent to the page hook. The page camera follows
/// the player's *interpolated render state* every frame (the same
/// snapshot-buffer sample the renderer draws models from), so the camera can
/// never lag or lead the rendered character. `fov_deg` is the *vertical*
/// field of view in degrees (Three.js convention); callers convert from the
/// engine's horizontal FOV using the requested aspect ratio.
#[derive(Debug, Clone)]
pub struct CaptureAttach {
    pub player_id: String,
    pub eye_height: f32,
    pub fov_deg: f32,
}

pub struct CaptureManager {
    page_url: String,
    inner: Mutex<Option<CaptureSession>>,
}

impl CaptureManager {
    /// `page_url` is the spectator viewport page, e.g.
    /// `http://127.0.0.1:8080/?viewport=1&spectator_token=...`.
    pub fn new(page_url: String) -> Self {
        Self {
            page_url,
            inner: Mutex::new(None),
        }
    }

    /// Capture a PNG frame of `attach.player_id`'s first-person view at
    /// `width`x`height`. Requests are serialized; the browser and page are
    /// (re)created on demand.
    pub async fn capture(
        &self,
        attach: &CaptureAttach,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        let mut guard = self.inner.lock().await;
        // One retry with a fresh browser if the existing session went bad
        // (browser crashed, socket closed, page reloaded).
        for attempt in 0..2 {
            if guard.is_none() {
                *guard = Some(CaptureSession::start(&self.page_url, width, height).await?);
            }
            let session = guard.as_mut().expect("session initialized above");
            match session.capture(attach, width, height).await {
                Ok(png) => return Ok(png),
                Err(err) if attempt == 0 => {
                    eprintln!("[capture] session error, restarting browser: {err}");
                    *guard = None;
                }
                Err(err) => return Err(err),
            }
        }
        unreachable!("capture loop always returns")
    }
}

struct CaptureSession {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    session_id: String,
    next_id: u64,
    cur_width: u32,
    cur_height: u32,
    // Kept for lifetime management: killed on drop.
    #[allow(dead_code)]
    child: Child,
    #[allow(dead_code)]
    user_data_dir: Option<tempdir::TempDirGuard>,
}

mod tempdir {
    use std::path::PathBuf;

    /// Minimal owned temp dir, removed on drop (best effort).
    pub struct TempDirGuard(pub PathBuf);

    impl TempDirGuard {
        pub fn create(prefix: &str) -> std::io::Result<Self> {
            let mut dir = std::env::temp_dir();
            dir.push(format!("{}-{}", prefix, uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir)?;
            Ok(Self(dir))
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

impl CaptureSession {
    async fn start(page_url: &str, width: u32, height: u32) -> Result<Self, String> {
        let browser = find_browser().ok_or_else(|| {
            "No Chromium/Chrome binary found for vision capture. Install Chromium or set \
                 CLAWBLOX_CHROME to a browser executable."
                .to_string()
        })?;

        let user_data_dir = tempdir::TempDirGuard::create("clawblox-capture")
            .map_err(|e| format!("Failed to create capture profile dir: {e}"))?;

        let mut command = Command::new(&browser);
        command
            .arg("--headless")
            .arg("--no-sandbox")
            .arg("--disable-dev-shm-usage")
            .arg("--hide-scrollbars")
            .arg("--mute-audio")
            .arg("--force-color-profile=srgb")
            .arg("--enable-unsafe-swiftshader")
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", user_data_dir.0.display()))
            .arg("about:blank")
            // Never let the browser attach to an X/Wayland display: software
            // WebGL (SwiftShader) is the portable, deterministic path.
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to launch capture browser {browser:?}: {e}"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Capture browser stderr unavailable".to_string())?;
        let mut lines = BufReader::new(stderr).lines();

        let ws_url = tokio::time::timeout(BROWSER_START_TIMEOUT, async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(rest) = line.trim().strip_prefix("DevTools listening on ") {
                    return Some(rest.trim().to_string());
                }
            }
            None
        })
        .await
        .map_err(|_| "Timed out waiting for capture browser DevTools endpoint".to_string())?
        .ok_or_else(|| "Capture browser exited before exposing DevTools endpoint".to_string())?;

        // Keep draining stderr so the browser never blocks on a full pipe.
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });

        let (ws, _) = connect_async(&ws_url)
            .await
            .map_err(|e| format!("Failed to connect to capture browser CDP: {e}"))?;

        let mut session = Self {
            ws,
            session_id: String::new(),
            next_id: 0,
            cur_width: 0,
            cur_height: 0,
            child,
            user_data_dir: Some(user_data_dir),
        };

        let target = session
            .command(None, "Target.createTarget", json!({ "url": page_url }))
            .await?;
        let target_id = target
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| "CDP Target.createTarget returned no targetId".to_string())?
            .to_string();

        let attached = session
            .command(
                None,
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
            )
            .await?;
        session.session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| "CDP Target.attachToTarget returned no sessionId".to_string())?
            .to_string();

        session.set_viewport(width, height).await?;
        session.wait_for_page_ready().await?;
        Ok(session)
    }

    async fn set_viewport(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self.cur_width == width && self.cur_height == height {
            return Ok(());
        }
        let session_id = self.session_id.clone();
        self.command(
            Some(&session_id),
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": width,
                "height": height,
                "deviceScaleFactor": 1,
                "mobile": false,
            }),
        )
        .await?;
        self.cur_width = width;
        self.cur_height = height;
        Ok(())
    }

    /// Wait until the page's capture hook reports a live camera API and at
    /// least one rendered frame (world renderer loaded, spectate WS streaming).
    async fn wait_for_page_ready(&mut self) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + PAGE_READY_TIMEOUT;
        loop {
            let status = self.eval_capture_hook("status()").await;
            if let Ok(status) = &status {
                let has_camera = status
                    .get("hasCamera")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let frame = status.get("frame").and_then(Value::as_u64).unwrap_or(0);
                if has_camera && frame > 0 {
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(match status {
                    Ok(status) => format!(
                        "Capture page did not become ready (renderer not drawing): {status}"
                    ),
                    Err(err) => format!("Capture page did not become ready: {err}"),
                });
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn capture(
        &mut self,
        attach: &CaptureAttach,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        self.set_viewport(width, height).await?;

        let attach_json = json!({
            "playerId": attach.player_id,
            "eyeHeight": attach.eye_height,
            "fovDeg": attach.fov_deg,
        });
        let result = self
            .eval_capture_hook(&format!("attach({attach_json})"))
            .await?;
        if !result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            return Err(format!("Capture page rejected camera attach: {result}"));
        }
        let base_frame = result.get("frame").and_then(Value::as_u64).unwrap_or(0);

        // Wait for the renderer to draw at least two frames with the camera
        // attached so the screenshot reflects the new viewpoint. The camera
        // tracks the player's interpolated state each frame, so waiting here
        // cannot introduce camera/body drift.
        let deadline = tokio::time::Instant::now() + FRAME_WAIT_TIMEOUT;
        loop {
            let status = self.eval_capture_hook("status()").await?;
            let frame = status.get("frame").and_then(Value::as_u64).unwrap_or(0);
            if frame >= base_frame + 2 {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err("Timed out waiting for renderer frame after camera pose".to_string());
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        let session_id = self.session_id.clone();
        let shot = self
            .command(
                Some(&session_id),
                "Page.captureScreenshot",
                json!({ "format": "png" }),
            )
            .await?;
        let data = shot
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| "CDP Page.captureScreenshot returned no data".to_string())?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| format!("Failed to decode screenshot payload: {e}"))
    }

    /// Evaluate `window.__clawbloxCapture.<call>` in the page and return the
    /// JSON result.
    async fn eval_capture_hook(&mut self, call: &str) -> Result<Value, String> {
        let expression = format!(
            "window.__clawbloxCapture ? window.__clawbloxCapture.{call} : {{ ok: false, error: 'capture hook missing' }}"
        );
        let session_id = self.session_id.clone();
        let result = self
            .command(
                Some(&session_id),
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
            )
            .await?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("Capture hook evaluation failed: {exception}"));
        }
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    async fn command(
        &mut self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            message["sessionId"] = Value::String(session_id.to_string());
        }
        self.ws
            .send(Message::Text(message.to_string().into()))
            .await
            .map_err(|e| format!("CDP send failed ({method}): {e}"))?;

        tokio::time::timeout(CDP_COMMAND_TIMEOUT, async {
            loop {
                let frame = self
                    .ws
                    .next()
                    .await
                    .ok_or_else(|| format!("CDP socket closed during {method}"))?
                    .map_err(|e| format!("CDP receive failed ({method}): {e}"))?;
                let text = match frame {
                    Message::Text(text) => text,
                    Message::Close(_) => return Err(format!("CDP socket closed during {method}")),
                    _ => continue,
                };
                let value: Value = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("id").and_then(Value::as_u64) != Some(id) {
                    continue; // event or reply to another command
                }
                if let Some(error) = value.get("error") {
                    return Err(format!("CDP {method} failed: {error}"));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
        })
        .await
        .map_err(|_| format!("CDP {method} timed out"))?
    }
}

/// Locate a Chromium/Chrome binary: `CLAWBLOX_CHROME`, then Playwright's
/// browser cache (headless shell preferred), then common system locations.
pub fn find_browser() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CLAWBLOX_CHROME") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(home) = home::home_dir() {
        let caches = [
            home.join(".cache/ms-playwright"),
            home.join("Library/Caches/ms-playwright"),
            home.join("AppData/Local/ms-playwright"),
        ];
        let candidates = [
            (
                "chromium_headless_shell-",
                "chrome-headless-shell-linux64/chrome-headless-shell",
            ),
            ("chromium_headless_shell-", "chrome-linux/headless_shell"),
            ("chromium_headless_shell-", "chrome-mac/headless_shell"),
            ("chromium_headless_shell-", "chrome-win/headless_shell.exe"),
            ("chromium-", "chrome-linux64/chrome"),
            ("chromium-", "chrome-linux/chrome"),
            (
                "chromium-",
                "chrome-mac/Chromium.app/Contents/MacOS/Chromium",
            ),
            ("chromium-", "chrome-win/chrome.exe"),
        ];
        for cache in caches.iter().filter(|c| c.is_dir()) {
            for (prefix, suffix) in candidates {
                if let Some(found) = newest_cache_entry(cache, prefix, suffix) {
                    return Some(found);
                }
            }
        }
    }

    let system = [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    ];
    for name in system {
        let path = PathBuf::from(name);
        if path.is_absolute() {
            if path.is_file() {
                return Some(path);
            }
        } else if let Some(found) = which(name) {
            return Some(found);
        }
    }
    None
}

/// Highest-versioned `ms-playwright/<prefix>NNNN/<suffix>` executable.
fn newest_cache_entry(cache: &PathBuf, prefix: &str, suffix: &str) -> Option<PathBuf> {
    let mut best: Option<(String, PathBuf)> = None;
    for entry in std::fs::read_dir(cache).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(prefix) {
            continue;
        }
        let executable = entry.path().join(suffix);
        if !executable.is_file() {
            continue;
        }
        if best.as_ref().map(|(b, _)| name > *b).unwrap_or(true) {
            best = Some((name, executable));
        }
    }
    best.map(|(_, path)| path)
}

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}
