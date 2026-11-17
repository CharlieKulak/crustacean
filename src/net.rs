//! Talking to the OpenSnake server, and saving settings, on every platform.
//!
//! Requests and WebSockets never block the game: they run in the background
//! (threads on desktop, the browser's fetch and WebSocket on the web) and their
//! results come back as events, collected once per frame with `poll()`.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum NetEvent {
    Http { id: u32, status: u16, body: String },
    WsOpen { id: u32 },
    WsMessage { id: u32, text: String },
    WsClosed { id: u32 },
    /// A picture the player picked (PNG bytes, base64), for their profile picture.
    Picked { data: String },
}

pub use platform::*;

/// Everything that's arrived since last frame.
pub fn poll() -> VecDeque<NetEvent> {
    platform::poll_events()
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::NetEvent;
    use std::collections::{HashMap, VecDeque};
    use std::io::ErrorKind;
    use std::net::TcpStream;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use tungstenite::stream::MaybeTlsStream;
    use tungstenite::{Message, WebSocket};

    struct Shared {
        tx: Sender<NetEvent>,
        rx: Receiver<NetEvent>,
        next: u32,
        sockets: HashMap<u32, Sender<Option<String>>>,
    }

    fn shared() -> &'static Mutex<Shared> {
        static S: OnceLock<Mutex<Shared>> = OnceLock::new();
        S.get_or_init(|| {
            let (tx, rx) = channel();
            Mutex::new(Shared { tx, rx, next: 1, sockets: HashMap::new() })
        })
    }

    fn next_id() -> (u32, Sender<NetEvent>) {
        let mut s = shared().lock().unwrap();
        s.next += 1;
        (s.next, s.tx.clone())
    }

    pub fn poll_events() -> VecDeque<NetEvent> {
        let s = shared().lock().unwrap();
        s.rx.try_iter().collect()
    }

    pub fn http(method: &str, url: &str, body: Option<String>, token: Option<&str>) -> u32 {
        let (id, tx) = next_id();
        let (method, url, token) = (method.to_string(), url.to_string(), token.map(String::from));
        std::thread::spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(20)))
                .http_status_as_error(false)
                .build()
                .into();
            let auth = token.map(|t| format!("Bearer {t}"));
            let result = if method == "POST" {
                let mut req = agent.post(&url).header("Content-Type", "application/json");
                if let Some(a) = &auth {
                    req = req.header("Authorization", a);
                }
                req.send(body.unwrap_or_else(|| "{}".into()))
            } else {
                let mut req = agent.get(&url);
                if let Some(a) = &auth {
                    req = req.header("Authorization", a);
                }
                req.call()
            };
            let ev = match result {
                Ok(mut resp) => {
                    let status = resp.status().as_u16();
                    let body = resp.body_mut().with_config().limit(8 * 1024 * 1024).read_to_string().unwrap_or_default();
                    NetEvent::Http { id, status, body }
                }
                Err(e) => NetEvent::Http { id, status: 0, body: format!("{{\"error\":\"{}\"}}", short(&e.to_string())) },
            };
            let _ = tx.send(ev);
        });
        id
    }

    fn short(s: &str) -> String {
        s.chars().filter(|c| *c != '"' && *c != '\\').take(120).collect()
    }

    pub fn ws_open(url: &str) -> u32 {
        let (id, tx) = next_id();
        let (out_tx, out_rx) = channel::<Option<String>>();
        shared().lock().unwrap().sockets.insert(id, out_tx);
        let url = url.to_string();
        std::thread::spawn(move || {
            let Ok((mut ws, _)) = tungstenite::connect(&url) else {
                let _ = tx.send(NetEvent::WsClosed { id });
                return;
            };
            set_timeout(&mut ws);
            let _ = tx.send(NetEvent::WsOpen { id });
            loop {
                // send what the game queued
                loop {
                    match out_rx.try_recv() {
                        Ok(Some(text)) => {
                            if ws.send(Message::text(text)).is_err() {
                                let _ = tx.send(NetEvent::WsClosed { id });
                                return;
                            }
                        }
                        Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            let _ = ws.close(None);
                            let _ = tx.send(NetEvent::WsClosed { id });
                            return;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    }
                }
                match ws.read() {
                    Ok(Message::Text(t)) => {
                        let _ = tx.send(NetEvent::WsMessage { id, text: t.to_string() });
                    }
                    Ok(Message::Close(_)) => {
                        let _ = tx.send(NetEvent::WsClosed { id });
                        return;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                    Err(_) => {
                        let _ = tx.send(NetEvent::WsClosed { id });
                        return;
                    }
                }
            }
        });
        id
    }

    fn set_timeout(ws: &mut WebSocket<MaybeTlsStream<TcpStream>>) {
        let t = Some(Duration::from_millis(4));
        match ws.get_mut() {
            MaybeTlsStream::Plain(s) => {
                let _ = s.set_read_timeout(t);
                let _ = s.set_nodelay(true);
            }
            MaybeTlsStream::Rustls(s) => {
                let _ = s.get_mut().set_read_timeout(t);
                let _ = s.get_mut().set_nodelay(true);
            }
            _ => {}
        }
    }

    pub fn ws_send(id: u32, text: &str) {
        if let Some(tx) = shared().lock().unwrap().sockets.get(&id) {
            let _ = tx.send(Some(text.to_string()));
        }
    }

    pub fn ws_close(id: u32) {
        if let Some(tx) = shared().lock().unwrap().sockets.remove(&id) {
            let _ = tx.send(None);
        }
    }

    fn settings_path() -> Option<std::path::PathBuf> {
        if let Ok(dir) = std::env::var("CRUSTACEAN_CONFIG_DIR") {
            return Some(std::path::PathBuf::from(dir).join("crustacean.json"));
        }
        directories::ProjectDirs::from("", "", "crustacean").map(|d| d.config_dir().join("crustacean.json"))
    }

    /// Where the settings were kept before Crustacean had its name.
    fn old_settings_path() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("", "", "opensnake").map(|d| d.config_dir().join("opensnake-rs.json"))
    }

    pub fn storage_get() -> Option<String> {
        let path = settings_path()?;
        match std::fs::read_to_string(&path) {
            Ok(s) => Some(s),
            // the first time: bring over the old settings (your login, pack, bests...)
            Err(_) if std::env::var("CRUSTACEAN_CONFIG_DIR").is_err() => std::fs::read_to_string(old_settings_path()?).ok(),
            Err(_) => None,
        }
    }

    pub fn storage_set(value: &str) {
        if let Some(p) = settings_path() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, value).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }

    // ---- one game per computer
    //
    // The running game holds a port on this computer only (127.0.0.1) and answers on it
    // that it's an OpenSnake game; a second one finds the port taken and asks who has it.

    const INSTANCE_PORT: u16 = 47817;
    const INSTANCE_REPLY: &str = "{\"opensnake-client\":\"crustacean\"}";

    fn held() -> &'static Mutex<bool> {
        static H: OnceLock<Mutex<bool>> = OnceLock::new();
        H.get_or_init(|| Mutex::new(false))
    }

    /// Is another OpenSnake game running on this computer? (Claims the spot if it's free.)
    pub fn other_instance() -> bool {
        use std::io::{Read, Write};
        use std::net::{SocketAddr, TcpListener};
        let mut h = held().lock().unwrap();
        if *h {
            return false;
        }
        let addr: SocketAddr = ([127, 0, 0, 1], INSTANCE_PORT).into();
        match TcpListener::bind(addr) {
            Ok(listener) => {
                *h = true;
                std::thread::spawn(move || {
                    for stream in listener.incoming().flatten() {
                        let mut stream = stream;
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                        let mut buf = [0u8; 1024];
                        let _ = stream.read(&mut buf);
                        let reply = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{INSTANCE_REPLY}",
                            INSTANCE_REPLY.len()
                        );
                        let _ = stream.write_all(reply.as_bytes());
                    }
                });
                false
            }
            Err(_) => {
                // Taken: by another OpenSnake game, or by some other program (then carry on).
                // (asked a few times: a game that's just starting may be slow to answer)
                for _ in 0..3 {
                    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
                        continue;
                    };
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
                    let _ = stream.write_all(b"GET / HTTP/1.0\r\n\r\n");
                    let mut reply = Vec::new();
                    let _ = stream.read_to_end(&mut reply);
                    if String::from_utf8_lossy(&reply).contains("opensnake-client") {
                        return true;
                    }
                    if !reply.is_empty() {
                        return false; // (something else answered: not ours)
                    }
                }
                false
            }
        }
    }

    /// Picking a picture needs a file dialog: on desktop, the player types the file's path.
    pub fn pick_image() {}

    pub fn read_image_file(path: &str) -> Option<Vec<u8>> {
        std::fs::read(path.trim().trim_matches('"')).ok()
    }

    /// The website the game was loaded from (none on desktop).
    pub fn page_origin() -> Option<String> {
        None
    }

    pub fn set_title(_t: &str) {}

    pub fn now_ms() -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    //! In the browser: see web/crustacean-net.js for the other half.
    use super::NetEvent;
    use std::collections::VecDeque;

    unsafe extern "C" {
        fn os_http(m: *const u8, ml: u32, u: *const u8, ul: u32, b: *const u8, bl: u32, t: *const u8, tl: u32) -> u32;
        fn os_ws_open(u: *const u8, ul: u32) -> u32;
        fn os_ws_send(id: u32, t: *const u8, tl: u32);
        fn os_ws_close(id: u32);
        fn os_event_len() -> i32;
        fn os_event_take(buf: *mut u8);
        fn os_storage_len() -> i32;
        fn os_storage_take(buf: *mut u8);
        fn os_storage_set(v: *const u8, vl: u32);
        fn os_pick_image();
        fn os_origin_len() -> i32;
        fn os_origin_take(buf: *mut u8);
        fn os_set_title(t: *const u8, tl: u32);
        fn os_now() -> f64;
        fn os_other_instance() -> i32;
    }

    /// Is another OpenSnake game open in this browser (another tab or window of this site)?
    pub fn other_instance() -> bool {
        unsafe { os_other_instance() != 0 }
    }

    /// (miniquad's loader checks the JS plugin matches this version)
    #[unsafe(no_mangle)]
    pub extern "C" fn crustacean_net_crate_version() -> u32 {
        1
    }

    fn take(len: i32, f: unsafe extern "C" fn(*mut u8)) -> Option<String> {
        if len < 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        unsafe { f(buf.as_mut_ptr()) };
        String::from_utf8(buf).ok()
    }

    pub fn poll_events() -> VecDeque<NetEvent> {
        let mut out = VecDeque::new();
        loop {
            let len = unsafe { os_event_len() };
            let Some(text) = take(len, os_event_take) else { break };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let id = v["id"].as_u64().unwrap_or(0) as u32;
            let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
            out.push_back(match v["k"].as_str().unwrap_or("") {
                "http" => NetEvent::Http { id, status: v["status"].as_u64().unwrap_or(0) as u16, body: s("body") },
                "open" => NetEvent::WsOpen { id },
                "msg" => NetEvent::WsMessage { id, text: s("text") },
                "picked" => NetEvent::Picked { data: s("data") },
                _ => NetEvent::WsClosed { id },
            });
        }
        out
    }

    pub fn http(method: &str, url: &str, body: Option<String>, token: Option<&str>) -> u32 {
        let b = body.unwrap_or_default();
        let t = token.unwrap_or("");
        unsafe {
            os_http(method.as_ptr(), method.len() as u32, url.as_ptr(), url.len() as u32, b.as_ptr(), b.len() as u32,
                    t.as_ptr(), t.len() as u32)
        }
    }

    pub fn ws_open(url: &str) -> u32 {
        unsafe { os_ws_open(url.as_ptr(), url.len() as u32) }
    }

    pub fn ws_send(id: u32, text: &str) {
        unsafe { os_ws_send(id, text.as_ptr(), text.len() as u32) }
    }

    pub fn ws_close(id: u32) {
        unsafe { os_ws_close(id) }
    }

    pub fn storage_get() -> Option<String> {
        take(unsafe { os_storage_len() }, os_storage_take)
    }

    pub fn storage_set(value: &str) {
        unsafe { os_storage_set(value.as_ptr(), value.len() as u32) }
    }

    pub fn pick_image() {
        unsafe { os_pick_image() }
    }

    pub fn read_image_file(_path: &str) -> Option<Vec<u8>> {
        None
    }

    pub fn page_origin() -> Option<String> {
        take(unsafe { os_origin_len() }, os_origin_take).filter(|s| s.starts_with("http"))
    }

    pub fn set_title(t: &str) {
        unsafe { os_set_title(t.as_ptr(), t.len() as u32) }
    }

    pub fn now_ms() -> f64 {
        unsafe { os_now() }
    }
}
