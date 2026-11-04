//! Discord's local RPC (ADR 0080 §2): the desktop app's IPC pipe (`\\.\pipe\discord-ipc-N` on Windows, the
//! `discord-ipc-N` socket under the runtime or temp folder elsewhere, Flatpak's and Snap's too), its frames and the
//! worker thread that owns it.
//!
//! A frame is `opcode: u32 LE, length: u32 LE` and `length` bytes of JSON. Opcodes: 0 handshake, 1 frame, 2 close,
//! 3 ping, 4 pong. The handshake `{"v": 1, "client_id": ID}` is answered by a `DISPATCH` of `READY`; then the game
//! sends `SET_ACTIVITY`, `SUBSCRIBE` (`ACTIVITY_JOIN`, `ACTIVITY_JOIN_REQUEST`) and the answers to join requests, and
//! Discord dispatches the events.

use std::io::{self, Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The pipes Discord opens, `discord-ipc-0` to `-9` (one per running Discord: stable, PTB, Canary).
const PIPES: u32 = 10;
/// A frame longer than this is no Discord frame.
const MAX_FRAME: usize = 64 * 1024;
/// Discord takes 5 activity updates per 20 s: one per 4 s at most, the latest wins.
pub const THROTTLE: Duration = Duration::from_secs(4);
/// The first retry after a failed or lost connection, doubled up to [`RETRY_MAX`].
const RETRY: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);
/// How long the handshake's `READY` may take.
const READY_WAIT: Duration = Duration::from_secs(3);

pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;

/// One frame as written on the pipe.
pub fn encode(op: u32, body: &Value) -> Vec<u8> {
    let body = body.to_string().into_bytes();
    let mut b = Vec::with_capacity(8 + body.len());
    b.extend_from_slice(&op.to_le_bytes());
    b.extend_from_slice(&(body.len() as u32).to_le_bytes());
    b.extend_from_slice(&body);
    b
}

/// The frame at the start of `r`.
pub fn read_frame(r: &mut impl Read) -> io::Result<(u32, Value)> {
    let mut head = [0; 8];
    r.read_exact(&mut head)?;
    let op = u32::from_le_bytes(head[..4].try_into().unwrap());
    let len = u32::from_le_bytes(head[4..].try_into().unwrap()) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("a {len}-byte frame")));
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    let v = serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok((op, v))
}

/// What Discord told the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Connected: the Discord user's name.
    Ready(String),
    /// The player pressed Join (or accepted an invite) on someone's activity: its join secret.
    Join(String),
    /// A friend asks to join this player's activity: their user id and name.
    JoinRequest { id: String, name: String },
    /// The connection is gone (Discord closed); it is retried.
    Lost,
}

/// What a dispatched frame means to the game (`None`: an answer to a command, or an event it doesn't take).
pub fn event_of(v: &Value) -> Option<Event> {
    if v["cmd"] != "DISPATCH" {
        if v["evt"] == "ERROR" {
            eprintln!("ig2-client: discord: {} {}", v["cmd"], v["data"]["message"]);
        }
        return None;
    }
    let d = &v["data"];
    let user_name = |u: &Value| u["global_name"].as_str().filter(|s| !s.is_empty()).or(u["username"].as_str()).unwrap_or("").to_string();
    match v["evt"].as_str()? {
        "READY" => Some(Event::Ready(user_name(&d["user"]))),
        "ACTIVITY_JOIN" => d["secret"].as_str().map(|s| Event::Join(s.to_string())),
        "ACTIVITY_JOIN_REQUEST" => Some(Event::JoinRequest { id: d["user"]["id"].as_str()?.to_string(), name: user_name(&d["user"]) }),
        _ => None,
    }
}

/// A command frame's body.
pub fn command(cmd: &str, args: Value, nonce: u64) -> Value {
    json!({ "cmd": cmd, "args": args, "nonce": nonce.to_string() })
}

/// `SUBSCRIBE` to `evt`.
pub fn subscribe(evt: &str, nonce: u64) -> Value {
    json!({ "cmd": "SUBSCRIBE", "evt": evt, "args": {}, "nonce": nonce.to_string() })
}

/// The pipe to the Discord app: blocking reads and writes, and whether a read would find bytes.
trait Pipe: Read + Write + Send {
    fn has_data(&mut self) -> io::Result<bool>;
}

#[cfg(unix)]
mod pipe {
    use std::io::{self, Read};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    /// The socket and the bytes [`super::Pipe::has_data`] read ahead (std has no stable `peek` on a Unix socket).
    pub struct Sock {
        s: UnixStream,
        ahead: Vec<u8>,
    }

    impl Sock {
        pub fn new(s: UnixStream) -> Self {
            Self { s, ahead: Vec::new() }
        }
    }

    impl io::Read for Sock {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.ahead.is_empty() {
                return self.s.read(buf);
            }
            let n = buf.len().min(self.ahead.len());
            buf[..n].copy_from_slice(&self.ahead[..n]);
            self.ahead.drain(..n);
            Ok(n)
        }
    }

    impl io::Write for Sock {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.s.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.s.flush()
        }
    }

    impl super::Pipe for Sock {
        fn has_data(&mut self) -> io::Result<bool> {
            if !self.ahead.is_empty() {
                return Ok(true);
            }
            self.s.set_nonblocking(true)?;
            let mut b = [0; 4096];
            let r = self.s.read(&mut b);
            self.s.set_nonblocking(false)?;
            match r {
                Ok(0) => Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => {
                    self.ahead.extend_from_slice(&b[..n]);
                    Ok(true)
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
                Err(e) => Err(e),
            }
        }
    }

    /// The folders Discord's socket may be in: the runtime folder or the temp folder, and the Flatpak and Snap
    /// builds' folders under them.
    pub fn dirs() -> Vec<PathBuf> {
        let base: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().filter_map(std::env::var_os).map(PathBuf::from).chain([PathBuf::from("/tmp")]).collect();
        let mut v = Vec::new();
        for b in base {
            for sub in ["", "app/com.discordapp.Discord", "app/com.discordapp.DiscordCanary", "snap.discord", "snap.discord-canary", ".flatpak/dev.vencord.Vesktop/xdg-run"] {
                let d = if sub.is_empty() { b.clone() } else { b.join(sub) };
                if !v.contains(&d) {
                    v.push(d);
                }
            }
        }
        v
    }

    pub fn open(n: u32) -> io::Result<Box<dyn super::Pipe>> {
        let mut last = io::Error::from(io::ErrorKind::NotFound);
        for d in dirs() {
            match UnixStream::connect(d.join(format!("discord-ipc-{n}"))) {
                Ok(s) => {
                    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
                    s.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
                    return Ok(Box::new(Sock::new(s)));
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

#[cfg(windows)]
mod pipe {
    use std::ffi::c_void;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn PeekNamedPipe(pipe: *mut c_void, buf: *mut c_void, size: u32, read: *mut u32, avail: *mut u32, left: *mut u32) -> i32;
    }

    impl super::Pipe for File {
        fn has_data(&mut self) -> io::Result<bool> {
            let mut avail = 0u32;
            // SAFETY: the handle is this open pipe's; no buffer is passed, only the count is written.
            let ok = unsafe { PeekNamedPipe(self.as_raw_handle(), std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut()) };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(avail > 0)
        }
    }

    pub fn open(n: u32) -> io::Result<Box<dyn super::Pipe>> {
        let f = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{n}"))?;
        Ok(Box::new(f))
    }
}

#[cfg(not(any(unix, windows)))]
mod pipe {
    pub fn open(_: u32) -> std::io::Result<Box<dyn super::Pipe>> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// What the game asks of the worker.
enum Msg {
    /// The activity to show (`None` clears it).
    Activity(Option<Value>),
    /// Accept or turn down a friend's join request.
    Answer { user: String, accept: bool },
}

/// The worker's handle: the game's side of the channels. Dropping it ends the worker (it clears the activity first).
pub struct Client {
    tx: Sender<Msg>,
    rx: Receiver<Event>,
}

impl Client {
    /// Start the worker for application `app`.
    pub fn start(app: &'static str) -> io::Result<Self> {
        let (tx, worker_rx) = mpsc::channel();
        let (worker_tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("discord".into()).spawn(move || Worker { app, rx: worker_rx, tx: worker_tx, link: None, nonce: 0 }.run())?;
        Ok(Self { tx, rx })
    }

    pub fn set(&self, activity: Option<Value>) {
        let _ = self.tx.send(Msg::Activity(activity));
    }

    pub fn answer(&self, user: String, accept: bool) {
        let _ = self.tx.send(Msg::Answer { user, accept });
    }

    /// The events since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

struct Worker {
    app: &'static str,
    rx: Receiver<Msg>,
    tx: Sender<Event>,
    link: Option<Box<dyn Pipe>>,
    nonce: u64,
}

impl Worker {
    fn run(mut self) {
        // The activity wanted, whether Discord has it, and when it last took one.
        let mut want: Option<Value> = None;
        let mut sent = false;
        let mut last_sent: Option<Instant> = None;
        let mut answers: Vec<(String, bool)> = Vec::new();
        let (mut retry_at, mut retry) = (Instant::now(), RETRY);
        loop {
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(m) => self.take(m, &mut want, &mut sent, &mut answers),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(m) = self.rx.try_recv() {
                self.take(m, &mut want, &mut sent, &mut answers);
            }
            if self.link.is_none() {
                if Instant::now() < retry_at {
                    continue;
                }
                match self.connect() {
                    Ok(user) => {
                        retry = RETRY;
                        sent = false;
                        last_sent = None;
                        let _ = self.tx.send(Event::Ready(user));
                    }
                    Err(_) => {
                        retry_at = Instant::now() + retry;
                        retry = (retry * 2).min(RETRY_MAX);
                        continue;
                    }
                }
            }
            if let Err(e) = self.step(&want, &mut sent, &mut last_sent, &mut answers) {
                if e.kind() != io::ErrorKind::UnexpectedEof {
                    eprintln!("ig2-client: discord: {e}");
                }
                self.link = None;
                retry_at = Instant::now() + retry;
                let _ = self.tx.send(Event::Lost);
            }
        }
        // The game is closing: clear the activity (Discord also clears it when the pipe closes).
        if let Some(l) = &mut self.link {
            self.nonce += 1;
            let _ = l.write_all(&encode(OP_FRAME, &command("SET_ACTIVITY", json!({ "pid": std::process::id() }), self.nonce)));
            let _ = l.write_all(&encode(OP_CLOSE, &json!({})));
        }
    }

    fn take(&self, m: Msg, want: &mut Option<Value>, sent: &mut bool, answers: &mut Vec<(String, bool)>) {
        match m {
            Msg::Activity(a) => {
                if *want != a {
                    *want = a;
                    *sent = false;
                }
            }
            Msg::Answer { user, accept } => answers.push((user, accept)),
        }
    }

    /// Open the first pipe that answers the handshake, and subscribe to the join events.
    fn connect(&mut self) -> io::Result<String> {
        let mut last = io::Error::from(io::ErrorKind::NotFound);
        for n in 0..PIPES {
            match pipe::open(n).and_then(|p| self.handshake(p)) {
                Ok(user) => return Ok(user),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn handshake(&mut self, mut p: Box<dyn Pipe>) -> io::Result<String> {
        p.write_all(&encode(OP_HANDSHAKE, &json!({ "v": 1, "client_id": self.app })))?;
        let until = Instant::now() + READY_WAIT;
        loop {
            if !p.has_data()? {
                if Instant::now() > until {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            match read_frame(&mut p)? {
                (OP_CLOSE, v) => return Err(io::Error::other(format!("refused: {}", v["message"]))),
                (OP_FRAME, v) => {
                    if let Some(Event::Ready(user)) = event_of(&v) {
                        for evt in ["ACTIVITY_JOIN", "ACTIVITY_JOIN_REQUEST"] {
                            self.nonce += 1;
                            p.write_all(&encode(OP_FRAME, &subscribe(evt, self.nonce)))?;
                        }
                        self.link = Some(p);
                        return Ok(user);
                    }
                }
                _ => {}
            }
        }
    }

    /// One round on a live pipe: Discord's frames in, the answers and the activity out.
    fn step(&mut self, want: &Option<Value>, sent: &mut bool, last_sent: &mut Option<Instant>, answers: &mut Vec<(String, bool)>) -> io::Result<()> {
        let l = self.link.as_mut().expect("connected");
        while l.has_data()? {
            match read_frame(l)? {
                (OP_PING, v) => l.write_all(&encode(OP_PONG, &v))?,
                (OP_CLOSE, _) => return Err(io::ErrorKind::UnexpectedEof.into()),
                (OP_FRAME, v) => {
                    if let Some(e) = event_of(&v) {
                        let _ = self.tx.send(e);
                    }
                }
                _ => {}
            }
        }
        for (user, accept) in answers.drain(..) {
            self.nonce += 1;
            let cmd = if accept { "SEND_ACTIVITY_JOIN_INVITE" } else { "CLOSE_ACTIVITY_REQUEST" };
            l.write_all(&encode(OP_FRAME, &command(cmd, json!({ "user_id": user }), self.nonce)))?;
        }
        if !*sent && last_sent.is_none_or(|t| t.elapsed() >= THROTTLE) {
            self.nonce += 1;
            let mut args = json!({ "pid": std::process::id() });
            if let Some(a) = want {
                args["activity"] = a.clone();
            }
            l.write_all(&encode(OP_FRAME, &command("SET_ACTIVITY", args, self.nonce)))?;
            *sent = true;
            *last_sent = Some(Instant::now());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let b = encode(OP_HANDSHAKE, &json!({ "v": 1, "client_id": "42" }));
        assert_eq!(&b[..4], &[0, 0, 0, 0]);
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize, b.len() - 8);
        let (op, v) = read_frame(&mut &b[..]).unwrap();
        assert_eq!((op, v["client_id"].as_str()), (OP_HANDSHAKE, Some("42")));
        // A frame cut short or too long is an error, not a hang.
        assert!(read_frame(&mut &b[..b.len() - 1]).is_err());
        let mut long = 1u32.to_le_bytes().to_vec();
        long.extend_from_slice(&(MAX_FRAME as u32 + 1).to_le_bytes());
        assert!(read_frame(&mut &long[..]).is_err());
    }

    #[test]
    fn dispatched_events() {
        let ready = json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "v": 1, "user": { "id": "1", "username": "ada", "global_name": "Ada" } } });
        assert_eq!(event_of(&ready), Some(Event::Ready("Ada".into())));
        let join = json!({ "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": { "secret": "ABCDE-FGHJK" } });
        assert_eq!(event_of(&join), Some(Event::Join("ABCDE-FGHJK".into())));
        let ask = json!({ "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST", "data": { "user": { "id": "7", "username": "bob", "global_name": null } } });
        assert_eq!(event_of(&ask), Some(Event::JoinRequest { id: "7".into(), name: "bob".into() }));
        // A command's answer is no event.
        assert_eq!(event_of(&json!({ "cmd": "SET_ACTIVITY", "evt": null, "data": {} })), None);
    }

    #[test]
    fn commands_carry_a_nonce() {
        let c = command("SET_ACTIVITY", json!({ "pid": 1 }), 3);
        assert_eq!((c["cmd"].as_str(), c["nonce"].as_str(), c["args"]["pid"].as_u64()), (Some("SET_ACTIVITY"), Some("3"), Some(1)));
        assert_eq!(subscribe("ACTIVITY_JOIN", 4)["evt"], "ACTIVITY_JOIN");
    }

    /// A Discord stand-in on a Unix socket: the worker's handshake, subscriptions, activity and a join coming back.
    #[cfg(unix)]
    #[test]
    fn talks_to_a_discord_stand_in() {
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("ig2-discord-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let listener = UnixListener::bind(dir.join("discord-ipc-0")).unwrap();
        let server = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let (op, hello) = read_frame(&mut s).unwrap();
            assert_eq!((op, hello["client_id"].as_str()), (OP_HANDSHAKE, Some("99")));
            s.write_all(&encode(OP_FRAME, &json!({ "cmd": "DISPATCH", "evt": "READY", "data": { "user": { "username": "ada" } } }))).unwrap();
            let subs: Vec<String> = (0..2).map(|_| read_frame(&mut s).unwrap().1["evt"].as_str().unwrap().to_string()).collect();
            assert_eq!(subs, ["ACTIVITY_JOIN", "ACTIVITY_JOIN_REQUEST"]);
            let (_, set) = read_frame(&mut s).unwrap();
            assert_eq!(set["cmd"], "SET_ACTIVITY");
            assert_eq!(set["args"]["activity"]["details"], "Testing");
            s.write_all(&encode(OP_FRAME, &json!({ "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": { "secret": "CODE" } }))).unwrap();
            // Keep the pipe open until the worker has read the join.
            let _ = read_frame(&mut s);
        });
        let p: Box<dyn Pipe> = Box::new(pipe::Sock::new(std::os::unix::net::UnixStream::connect(dir.join("discord-ipc-0")).unwrap()));
        let (tx, rx) = mpsc::channel();
        let (_mtx, mrx) = mpsc::channel();
        let mut w = Worker { app: "99", rx: mrx, tx, link: None, nonce: 0 };
        assert_eq!(w.handshake(p).unwrap(), "ada");
        let want = Some(json!({ "details": "Testing" }));
        let (mut sent, mut last, mut answers) = (false, None, Vec::new());
        let until = Instant::now() + Duration::from_secs(5);
        let mut events = Vec::new();
        while events.is_empty() && Instant::now() < until {
            w.step(&want, &mut sent, &mut last, &mut answers).unwrap();
            events.extend(rx.try_iter());
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(sent);
        assert_eq!(events, [Event::Join("CODE".into())]);
        drop(w);
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
