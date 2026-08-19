use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::thread;
use crate::audio::AudioFeatures;

#[derive(Debug, Clone, Default)]
pub struct WebBridgeState {
    pub version: usize,
    pub sketch_source: String,
    pub audio_path: Option<PathBuf>,
    pub live_audio: AudioFeatures,
    pub is_playing: bool,
    pub current_frame: usize,
}

pub struct WebServer {
    #[allow(dead_code)]
    pub port: u16,
    state: Arc<RwLock<WebBridgeState>>,
}

impl WebServer {
    pub fn start(initial_port: u16, state: Arc<RwLock<WebBridgeState>>) -> Self {
        let mut port = initial_port;
        let listener = loop {
            match TcpListener::bind(format!("127.0.0.1:{}", port)) {
                Ok(l) => break l,
                Err(_) => {
                    port += 1;
                    if port > initial_port + 50 {
                        panic!("Failed to bind web server port");
                    }
                }
            }
        };

        let thread_state = Arc::clone(&state);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let st = Arc::clone(&thread_state);
                thread::spawn(move || {
                    let _ = handle_connection(stream, st);
                });
            }
        });

        Self { port, state }
    }

    pub fn update_state(&self, version: usize, source: &str, audio_path: Option<PathBuf>, features: AudioFeatures, is_playing: bool, frame: usize) {
        if let Ok(mut lock) = self.state.write() {
            lock.version = version;
            lock.sketch_source = source.to_string();
            lock.audio_path = audio_path;
            lock.live_audio = features;
            lock.is_playing = is_playing;
            lock.current_frame = frame;
        }
    }
}

fn handle_connection(mut stream: TcpStream, state: Arc<RwLock<WebBridgeState>>) -> std::io::Result<()> {
    let mut buffer = [0u8; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 2 {
        return Ok(());
    }

    let method = parts[0];
    let path = parts[1];

    if method != "GET" {
        let response = "HTTP/1.1 405 Method Not Allowed\r\n\r\n";
        stream.write_all(response.as_bytes())?;
        return Ok(());
    }

    let clean_path = path.split('?').next().unwrap_or("/");

    match clean_path {
        "/" | "/index.html" => {
            let html = INDEX_HTML;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                html.len(),
                html
            );
            stream.write_all(response.as_bytes())?;
        }
        "/sketch.js" => {
            let code = {
                let lock = state.read().unwrap();
                lock.sketch_source.clone()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/javascript; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                code.len(),
                code
            );
            stream.write_all(response.as_bytes())?;
        }
        "/api/state" => {
            let (ver, audio, is_playing, frame) = {
                let lock = state.read().unwrap();
                (lock.version, lock.live_audio, lock.is_playing, lock.current_frame)
            };
            let json = serde_json::json!({
                "version": ver,
                "is_playing": is_playing,
                "current_frame": frame,
                "audio": {
                    "amplitude": audio.amplitude,
                    "low": audio.low,
                    "mid": audio.mid,
                    "high": audio.high
                }
            }).to_string();

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                json.len(),
                json
            );
            stream.write_all(response.as_bytes())?;
        }
        "/audio" => {
            let audio_path = {
                let lock = state.read().unwrap();
                lock.audio_path.clone()
            };

            if let Some(path) = audio_path {
                if let Ok(mut file) = File::open(&path) {
                    let mut data = Vec::new();
                    if file.read_to_end(&mut data).is_ok() {
                        let mime = if path.extension().and_then(|e| e.to_str()) == Some("mp3") {
                            "audio/mpeg"
                        } else {
                            "audio/wav"
                        };
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccept-Ranges: bytes\r\n\r\n",
                            mime,
                            data.len()
                        );
                        stream.write_all(header.as_bytes())?;
                        stream.write_all(&data)?;
                        return Ok(());
                    }
                }
            }

            let response = "HTTP/1.1 404 Not Found\r\n\r\n";
            stream.write_all(response.as_bytes())?;
        }
        _ => {
            let response = "HTTP/1.1 404 Not Found\r\n\r\n";
            stream.write_all(response.as_bytes())?;
        }
    }

    Ok(())
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Grain — Native p5.js High-Res Creative Canvas</title>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/p5.js/1.9.0/p5.min.js"></script>
  <style>
    * { box-sizing: border-box; }
    body, html {
      margin: 0;
      padding: 0;
      width: 100%;
      height: 100%;
      overflow: hidden;
      background: #09090b;
      display: flex;
      justify-content: center;
      align-items: center;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      color: #fafafa;
    }
    #canvas-wrapper {
      position: relative;
      box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.8), 0 0 40px rgba(6, 182, 212, 0.15);
      border-radius: 16px;
      overflow: hidden;
      border: 1px solid rgba(255, 255, 255, 0.12);
      background: #000;
    }
    #hud {
      position: absolute;
      top: 20px;
      left: 20px;
      padding: 10px 16px;
      background: rgba(15, 23, 42, 0.85);
      backdrop-filter: blur(12px);
      border-radius: 10px;
      font-size: 13px;
      display: flex;
      gap: 16px;
      align-items: center;
      border: 1px solid rgba(255, 255, 255, 0.15);
      z-index: 10;
      box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    }
    .badge {
      background: linear-gradient(135deg, #06b6d4, #3b82f6);
      color: #000;
      font-weight: 800;
      letter-spacing: 0.5px;
      padding: 3px 8px;
      border-radius: 6px;
      font-size: 11px;
    }
    #meters {
      display: flex;
      gap: 12px;
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      font-size: 12px;
    }
    .meter-val { font-weight: bold; }
  </style>
</head>
<body>
  <div id="hud">
    <span class="badge">GRAIN LIVE CANVAS</span>
    <span id="version-label" style="font-weight: 600; color: #38bdf8;">Connecting...</span>
    <div id="meters">
      <span>BASS <span id="m-low" class="meter-val" style="color:#06b6d4">0.00</span></span>
      <span>MID <span id="m-mid" class="meter-val" style="color:#eab308">0.00</span></span>
      <span>HIGH <span id="m-high" class="meter-val" style="color:#22c55e">0.00</span></span>
      <span>AMP <span id="m-amp" class="meter-val" style="color:#ec4899">0.00</span></span>
    </div>
  </div>

  <div id="canvas-wrapper">
    <div id="p5-target"></div>
  </div>

  <script>
    let currentVersion = -1;
    let activeP5Instance = null;
    let liveFeatures = { amplitude: 0, low: 0, mid: 0, high: 0 };

    async function pollState() {
      try {
        const res = await fetch('/api/state');
        if (res.ok) {
          const data = await res.json();
          liveFeatures = data.audio;
          document.getElementById('m-low').innerText = (data.audio.low || 0).toFixed(2);
          document.getElementById('m-mid').innerText = (data.audio.mid || 0).toFixed(2);
          document.getElementById('m-high').innerText = (data.audio.high || 0).toFixed(2);
          document.getElementById('m-amp').innerText = (data.audio.amplitude || 0).toFixed(2);

          if (data.version !== currentVersion) {
            currentVersion = data.version;
            document.getElementById('version-label').innerText = 'sketch_v' + currentVersion;
            await reloadSketch();
          }
        }
      } catch (e) {}
    }

    setInterval(pollState, 100);

    async function reloadSketch() {
      try {
        const res = await fetch('/sketch.js?t=' + Date.now());
        if (!res.ok) return;
        const code = await res.text();

        if (activeP5Instance) {
          activeP5Instance.remove();
        }

        const sketchWrapper = (p) => {
          let frameIndex = 0;
          let startTime = performance.now();

          p.setup = () => {
            const w = Math.min(window.innerWidth - 80, 960);
            const h = Math.min(window.innerHeight - 80, 640);
            p.createCanvas(w, h);
            p.frameRate(60);

            try {
              const runSetup = new Function('p', code + '\nif (typeof setup === "function") setup(p);');
              runSetup(p);
            } catch (err) {
              console.error("p5 setup() error:", err);
            }
          };

          p.draw = () => {
            frameIndex++;
            const elapsed = (performance.now() - startTime) / 1000.0;
            const ctx = {
              width: p.width,
              height: p.height,
              frame: frameIndex,
              time: elapsed,
              seed: 42,
              audio: liveFeatures
            };

            try {
              const runDraw = new Function('p', 'ctx', code + '\nif (typeof draw === "function") draw(p, ctx);');
              runDraw(p, ctx);
            } catch (err) {
              console.error("p5 draw() error:", err);
            }
          };

          p.windowResized = () => {
            const w = Math.min(window.innerWidth - 80, 960);
            const h = Math.min(window.innerHeight - 80, 640);
            p.resizeCanvas(w, h);
          };
        };

        activeP5Instance = new p5(sketchWrapper, 'p5-target');
      } catch (e) {
        console.error("Failed to load sketch code:", e);
      }
    }
  </script>
</body>
</html>
"#;
