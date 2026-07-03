//! `gol serve` — a zero-dependency HTTP + WebSocket server that streams a
//! shared Game of Life simulation to any number of browser clients.
//!
//! One background thread ticks a single shared world; every connection reads
//! the latest generation and pushes it out. That one design demonstrates a
//! surprising number of backend concepts in real, running code:
//!
//! | Endpoint            | Concept it demonstrates                         |
//! |---------------------|-------------------------------------------------|
//! | `GET /ws`           | **WebSockets** (full duplex, RFC 6455 by hand)  |
//! | `GET /api/stream`   | **long polling** / streaming (Server-Sent Events)|
//! | `GET /api/state`    | **short polling** (stateless JSON snapshot)     |
//! | text frames in `/ws`| **RPC** (pause/reseed/speed control commands)   |
//! | `GET /healthz`      | **availability** / liveness probe               |
//! | `GET /readyz`       | readiness probe (for **load balancers**/k8s)    |
//! | `GET /metrics`      | **throughput / QPS** counters (Prometheus text) |
//! | token bucket        | **rate limiting** (per client IP)               |
//! | JSON logs to stderr | **error logging** / structured logging          |

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::wire;
use crate::{Grid, Rng, seed_pattern};

const WEB_CLIENT: &str = include_str!("../web/index.html");

/// Configuration for `gol serve`.
pub struct ServerConfig {
    pub addr: String,
    pub w: usize,
    pub h: usize,
    pub delay_ms: u64,
    pub pattern: String,
    pub seed: u64,
    pub density: f64,
    /// Token-bucket capacity (burst) for new connections per IP.
    pub rate_burst: f64,
    /// Token-bucket refill rate (connections/second) per IP.
    pub rate_per_sec: f64,
}

/// State shared between the ticker thread and every connection thread.
struct Shared {
    grid: Mutex<Grid>,
    generation: AtomicU64,
    delay_ms: AtomicU64,
    paused: AtomicBool,
    /// A pending reseed request: pattern name set by an RPC command.
    reseed: Mutex<Option<String>>,
    seed: AtomicU64,
    density_milli: AtomicU64, // density * 1000, since atomics can't hold f64
    // ---- metrics ----
    http_requests: AtomicU64,
    ws_connections: AtomicU64,
    active_connections: AtomicU64,
    frames_sent: AtomicU64,
    rate_limited: AtomicU64,
    started: Instant,
    // ---- rate limiting ----
    buckets: Mutex<HashMap<IpAddr, (f64, Instant)>>,
    rate_burst: f64,
    rate_per_sec: f64,
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Minimal structured (JSON-lines) logger to stderr.
fn log(level: &str, msg: &str, fields: &[(&str, String)]) {
    let mut line = format!(
        "{{\"ts\":{},\"level\":\"{}\",\"msg\":\"{}\"",
        now_millis(),
        level,
        msg
    );
    for (k, v) in fields {
        line.push_str(&format!(",\"{k}\":{v}"));
    }
    line.push('}');
    eprintln!("{line}");
}

/// Wire representation of one frame: `gen|w|h|pop|<base64 bitset>`.
fn frame_payload(shared: &Shared) -> String {
    let grid = shared.grid.lock().unwrap();
    let generation = shared.generation.load(Ordering::Relaxed);
    let bits = wire::base64_encode(&grid.pack());
    format!(
        "{}|{}|{}|{}|{}",
        generation,
        grid.w,
        grid.h,
        grid.population(),
        bits
    )
}

/// Start the server. Blocks forever (until the process is killed).
pub fn serve(cfg: ServerConfig) -> std::io::Result<()> {
    let mut grid = Grid::new(cfg.w, cfg.h);
    let mut rng = Rng::new(cfg.seed);
    seed_pattern(&mut grid, &cfg.pattern, &mut rng, cfg.density)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let shared = Arc::new(Shared {
        grid: Mutex::new(grid),
        generation: AtomicU64::new(0),
        delay_ms: AtomicU64::new(cfg.delay_ms),
        paused: AtomicBool::new(false),
        reseed: Mutex::new(None),
        seed: AtomicU64::new(cfg.seed),
        density_milli: AtomicU64::new((cfg.density * 1000.0) as u64),
        http_requests: AtomicU64::new(0),
        ws_connections: AtomicU64::new(0),
        active_connections: AtomicU64::new(0),
        frames_sent: AtomicU64::new(0),
        rate_limited: AtomicU64::new(0),
        started: Instant::now(),
        buckets: Mutex::new(HashMap::new()),
        rate_burst: cfg.rate_burst,
        rate_per_sec: cfg.rate_per_sec,
    });

    // ---- background ticker: advances the one shared world ----
    {
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            loop {
                let delay = shared.delay_ms.load(Ordering::Relaxed).max(1);
                thread::sleep(Duration::from_millis(delay));
                if shared.paused.load(Ordering::Relaxed) {
                    continue;
                }
                let mut grid = shared.grid.lock().unwrap();
                if let Some(pattern) = shared.reseed.lock().unwrap().take() {
                    for x in 0..grid.w {
                        for y in 0..grid.h {
                            grid.set(x, y, false);
                        }
                    }
                    let seed = shared.seed.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
                    let mut rng = Rng::new(seed);
                    let density = shared.density_milli.load(Ordering::Relaxed) as f64 / 1000.0;
                    let _ = seed_pattern(&mut grid, &pattern, &mut rng, density);
                    shared.generation.store(0, Ordering::Relaxed);
                } else {
                    grid.step();
                    shared.generation.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }

    let listener = TcpListener::bind(&cfg.addr)?;
    log(
        "info",
        "server listening",
        &[
            ("addr", format!("\"{}\"", cfg.addr)),
            ("grid", format!("\"{}x{}\"", cfg.w, cfg.h)),
        ],
    );
    println!(
        "gol-rs server on http://{} — open it in a browser",
        cfg.addr
    );

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                log("error", "accept failed", &[("err", format!("\"{e}\""))]);
                continue;
            }
        };
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            if let Err(e) = handle(stream, shared) {
                log("error", "connection ended", &[("err", format!("\"{e}\""))]);
            }
        });
    }
    Ok(())
}

/// Refill and consume one token from the caller's bucket. Returns `true` if the
/// request is allowed.
fn allow(shared: &Shared, ip: IpAddr) -> bool {
    let mut buckets = shared.buckets.lock().unwrap();
    let now = Instant::now();
    // Evict buckets idle for over a minute once the map grows, so a churn of
    // distinct client IPs can't grow it without bound.
    if buckets.len() > 1024 {
        buckets.retain(|_, (_, seen)| now.duration_since(*seen) < Duration::from_secs(60));
    }
    let entry = buckets.entry(ip).or_insert((shared.rate_burst, now));
    let elapsed = now.duration_since(entry.1).as_secs_f64();
    entry.1 = now;
    entry.0 = (entry.0 + elapsed * shared.rate_per_sec).min(shared.rate_burst);
    if entry.0 >= 1.0 {
        entry.0 -= 1.0;
        true
    } else {
        false
    }
}

fn handle(stream: TcpStream, shared: Arc<Shared>) -> std::io::Result<()> {
    let peer = stream
        .peer_addr()
        .map(|a| a.ip())
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));
    let mut reader = BufReader::new(stream.try_clone()?);

    // ---- parse the request line + headers ----
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(()); // client hung up
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }

    shared.http_requests.fetch_add(1, Ordering::Relaxed);

    if !allow(&shared, peer) {
        shared.rate_limited.fetch_add(1, Ordering::Relaxed);
        log("warn", "rate limited", &[("ip", format!("\"{peer}\""))]);
        return write_simple(stream, 429, "text/plain", "429 Too Many Requests\n");
    }

    let route = path.split('?').next().unwrap_or("/");
    match (method.as_str(), route) {
        ("GET", "/") => write_simple(stream, 200, "text/html; charset=utf-8", WEB_CLIENT),
        ("GET", "/healthz") => write_simple(stream, 200, "text/plain", "ok\n"),
        ("GET", "/readyz") => {
            // Ready once the ticker has produced at least the initial board.
            write_simple(stream, 200, "text/plain", "ready\n")
        }
        ("GET", "/metrics") => {
            write_simple(stream, 200, "text/plain; version=0.0.4", &metrics(&shared))
        }
        ("GET", "/api/state") => {
            let grid = shared.grid.lock().unwrap();
            let generation = shared.generation.load(Ordering::Relaxed);
            let body = format!(
                "{{\"gen\":{},\"w\":{},\"h\":{},\"pop\":{},\"bits\":\"{}\"}}",
                generation,
                grid.w,
                grid.h,
                grid.population(),
                wire::base64_encode(&grid.pack())
            );
            drop(grid);
            write_simple(stream, 200, "application/json", &body)
        }
        ("GET", "/api/stream") => stream_sse(stream, shared),
        ("GET", "/ws") => {
            if let Some(key) = headers.get("sec-websocket-key") {
                let accept = wire::ws_accept_key(key);
                websocket(stream, reader, shared, &accept)
            } else {
                write_simple(stream, 400, "text/plain", "expected websocket upgrade\n")
            }
        }
        _ => write_simple(stream, 404, "text/plain", "404 Not Found\n"),
    }
}

fn metrics(shared: &Shared) -> String {
    let uptime = shared.started.elapsed().as_secs_f64();
    format!(
        "# HELP gol_http_requests_total Total HTTP requests.\n\
         # TYPE gol_http_requests_total counter\n\
         gol_http_requests_total {}\n\
         # HELP gol_ws_connections_total Total WebSocket upgrades.\n\
         # TYPE gol_ws_connections_total counter\n\
         gol_ws_connections_total {}\n\
         # HELP gol_active_connections Currently streaming connections.\n\
         # TYPE gol_active_connections gauge\n\
         gol_active_connections {}\n\
         # HELP gol_frames_sent_total Simulation frames pushed to clients.\n\
         # TYPE gol_frames_sent_total counter\n\
         gol_frames_sent_total {}\n\
         # HELP gol_generations_total Simulation generations computed.\n\
         # TYPE gol_generations_total counter\n\
         gol_generations_total {}\n\
         # HELP gol_rate_limited_total Requests rejected by the rate limiter.\n\
         # TYPE gol_rate_limited_total counter\n\
         gol_rate_limited_total {}\n\
         # HELP gol_uptime_seconds Process uptime.\n\
         # TYPE gol_uptime_seconds gauge\n\
         gol_uptime_seconds {:.1}\n",
        shared.http_requests.load(Ordering::Relaxed),
        shared.ws_connections.load(Ordering::Relaxed),
        shared.active_connections.load(Ordering::Relaxed),
        shared.frames_sent.load(Ordering::Relaxed),
        shared.generation.load(Ordering::Relaxed),
        shared.rate_limited.load(Ordering::Relaxed),
        uptime,
    )
}

/// Server-Sent Events: a long-lived HTTP response that streams frames. This is
/// the "long polling"/streaming fallback for clients without WebSockets.
fn stream_sse(mut stream: TcpStream, shared: Arc<Shared>) -> std::io::Result<()> {
    let header = "HTTP/1.1 200 OK\r\n\
                  Content-Type: text/event-stream\r\n\
                  Cache-Control: no-cache\r\n\
                  Connection: keep-alive\r\n\
                  Access-Control-Allow-Origin: *\r\n\r\n";
    stream.write_all(header.as_bytes())?;
    shared.active_connections.fetch_add(1, Ordering::Relaxed);
    let _guard = ConnGuard(&shared);

    let mut last = u64::MAX;
    loop {
        let generation = shared.generation.load(Ordering::Relaxed);
        if generation != last {
            last = generation;
            let payload = frame_payload(&shared);
            if stream
                .write_all(format!("data: {payload}\n\n").as_bytes())
                .is_err()
            {
                break;
            }
            stream.flush().ok();
            shared.frames_sent.fetch_add(1, Ordering::Relaxed);
        }
        thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}

/// A full-duplex WebSocket connection: we stream frames out and accept plain
/// text control commands in (an RPC channel: pause/resume/reseed/faster/slower).
fn websocket(
    mut stream: TcpStream,
    mut reader: BufReader<TcpStream>,
    shared: Arc<Shared>,
    accept: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    );
    stream.write_all(response.as_bytes())?;
    shared.ws_connections.fetch_add(1, Ordering::Relaxed);
    shared.active_connections.fetch_add(1, Ordering::Relaxed);
    let _guard = ConnGuard(&shared);
    log(
        "info",
        "ws connected",
        &[(
            "active",
            format!("{}", shared.active_connections.load(Ordering::Relaxed)),
        )],
    );

    // ---- control reader thread (RPC in) ----
    {
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            let mut buf: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let n = match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&chunk[..n]);
                while let Some((frame, used)) = wire::parse_client_frame(&buf) {
                    buf.drain(..used);
                    match frame.opcode {
                        0x8 => return, // close
                        0x1 => {
                            let cmd = String::from_utf8_lossy(&frame.payload).trim().to_string();
                            apply_command(&shared, &cmd);
                        }
                        _ => {}
                    }
                }
            }
        });
    }

    // ---- streaming writer (frames out) ----
    let mut last = u64::MAX;
    loop {
        let generation = shared.generation.load(Ordering::Relaxed);
        if generation != last {
            last = generation;
            let payload = frame_payload(&shared);
            if stream
                .write_all(&wire::ws_text_frame(payload.as_bytes()))
                .is_err()
            {
                break;
            }
            stream.flush().ok();
            shared.frames_sent.fetch_add(1, Ordering::Relaxed);
        }
        thread::sleep(Duration::from_millis(16));
    }
    let _ = stream.write_all(&wire::ws_close_frame());
    Ok(())
}

/// Apply a control command received over the WebSocket RPC channel.
fn apply_command(shared: &Shared, cmd: &str) {
    match cmd {
        "pause" => shared.paused.store(true, Ordering::Relaxed),
        "resume" => shared.paused.store(false, Ordering::Relaxed),
        "faster" => {
            let d = shared.delay_ms.load(Ordering::Relaxed);
            shared
                .delay_ms
                .store((d.saturating_sub(20)).max(1), Ordering::Relaxed);
        }
        "slower" => {
            let d = shared.delay_ms.load(Ordering::Relaxed);
            shared.delay_ms.store((d + 20).min(2000), Ordering::Relaxed);
        }
        "reseed" | "random" | "glider" | "pulsar" | "gosper" => {
            let pattern = if cmd == "reseed" { "random" } else { cmd };
            *shared.reseed.lock().unwrap() = Some(pattern.to_string());
        }
        other => log(
            "warn",
            "unknown command",
            &[("cmd", format!("\"{other}\""))],
        ),
    }
}

/// Decrements the active-connection gauge when a streaming handler returns.
struct ConnGuard<'a>(&'a Shared);
impl Drop for ConnGuard<'_> {
    fn drop(&mut self) {
        self.0.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

fn write_simple(
    mut stream: TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}
