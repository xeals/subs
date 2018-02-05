use crossbeam_channel::*;
use serde_json;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use sunk::Client;
use sunk::search::{self, SearchPage};
use unix_socket::{UnixListener, UnixStream};

use error::{Error, Result};
use player::Player;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Add(u64),
    AddSearch(String),
    Search(String, bool, bool, bool),
    StatusReq,
    Status(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Reply {
    Search {
        albums: Vec<String>,
        artists: Vec<String>,
        songs: Vec<String>,
    },
    Other(String),
    None,
}

pub struct Daemon {
    client: Arc<Mutex<Client>>,
    socket: UnixListener,
    cache_file: PathBuf,

    player_send: Sender<Command>,
    player_recv: Receiver<Command>,
}

impl Daemon {
    fn new(cfg: ::config::Config) -> Self {
        let _client = Client::new(&cfg.url, &cfg.username, &cfg.password)
            .expect("error starting server");
        let client = Arc::new(Mutex::new(_client));

        let cache_file = cfg.cache.clone();

        let (player_send, daemon_recv) = unbounded();
        let (daemon_send, player_recv) = unbounded();
        let player_cli = client.clone();
        thread::spawn(|| {
            let mut player = Player::new(daemon_recv, daemon_send, player_cli);

            player.run();
        });

        let socket = UnixListener::bind(cfg.socket.clone())
            .expect("unable to bind to socket");

        ::ctrlc::set_handler(move || {
            warn!("received SIGINT/TERM, shutting down");
            let sock_path = cfg.socket.clone();
            let mut stream = UnixStream::connect(sock_path)
                .expect("unable to connect to socket in <C-c> handler");
            stream
                .write_all(
                    serde_json::to_string(&Command::Stop)
                        .expect("error serializing Command::Stop")
                        .as_bytes(),
                )
                .expect("error writing to stream")
        }).expect("error starting <C-c> handler");

        Daemon {
            client,
            socket,
            cache_file,
            player_send,
            player_recv,
        }
    }

    fn run(&self) -> Result {
        for stream in self.socket.incoming() {
            match stream {
                Ok(mut s) => {
                    let mut buf = String::new();
                    s.read_to_string(&mut buf)?;
                    let cmd = serde_json::from_str::<Command>(&buf)?;

                    debug!("running {:?}", cmd);
                    match self.run_cmd(cmd) {
                        Err(Error::ExplicitStop) => {
                            info!("stop signal received.");
                            info!("stopping server.");
                            self.player_send.send(Command::Stop).unwrap();
                            break
                        }
                        Err(e) => {
                            error!("Encountered error: {}", e);
                            // return Err(e)
                        }
                        Ok(res) => {
                            s.write_all(res.as_bytes())?;
                            debug!("sent reply");
                        }
                    };
                }
                Err(e) => {
                    error!("encountered error: {}", e);
                    error!("stopping...");
                    return Err("Encountered error in stream".into())
                }
            }
        }

        Ok(())
    }

    fn run_cmd(&self, cmd: Command) -> ::std::result::Result<String, Error> {
        use self::Command::*;
        match cmd {
            Stop => return Err(Error::ExplicitStop),
            Play | Pause | Toggle => self.player_send.send(cmd).unwrap(),
            AddSearch(q) => {
                let n = search::NONE;
                let s = SearchPage::new().with_size(1);
                let sr = self.client.lock().unwrap().search(&q, n, n, s)?;
                return Ok(serde_json::to_string(&Reply::Other(if let Some(
                    song,
                ) =
                    sr.songs.get(0)
                {
                    self.player_send.send(Add(song.id)).unwrap();
                    "".into()
                } else {
                    format!("Nothing found for \"{}\"", q)
                }))?)
            }
            StatusReq => {
                self.player_send.send(Command::StatusReq).unwrap();
                let st = self.player_recv.recv().unwrap();
                if let Command::Status(s) = st {
                    return Ok(serde_json::to_string(&Reply::Other(s))?)
                } else {
                    unreachable!()
                }
            }
            Search(q, r, a, s) => {
                macro_rules! chk {
                    ($t:ident) => (if $t {
                        SearchPage::new()
                    } else {
                        search::NONE
                    });
                }

                debug!("searching");
                let ar = chk!(r);
                let al = chk!(a);
                let sn = chk!(s);
                let sr = self.client.lock().unwrap().search(&q, ar, al, sn)?;
                debug!("serializing reply");
                let res = Reply::Search {
                    albums: sr.albums.iter().map(|a| a.to_string()).collect(),
                    artists: sr.artists.iter().map(|a| a.to_string()).collect(),
                    songs: sr.songs.iter().map(|s| s.to_string()).collect(),
                };
                return Ok(serde_json::to_string(&res)?)
            }
            _ => (),
        }
        Ok("".into())
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        ::std::fs::remove_file(
            self.socket.local_addr().unwrap().as_pathname().unwrap(),
        ).unwrap();
    }
}

pub fn send(cmd: Command) -> Result {
    let cfg = ::config()?;
    let mut stream = UnixStream::connect(cfg.socket)?;

    let json = serde_json::to_string(&cmd)?;
    debug!("sending {}", json);
    stream.write_all(json.as_bytes())?;

    Ok(())
}

pub fn send_recv(cmd: Command) -> ::std::result::Result<Reply, Error> {
    let cfg = ::config()?;
    let mut stream = UnixStream::connect(cfg.socket)?;

    let json = serde_json::to_string(&cmd)?;
    debug!("sending {}", json);
    stream.write_all(json.as_bytes())?;

    use std::net::Shutdown;
    stream.shutdown(Shutdown::Write)?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(serde_json::from_str::<Reply>(&reply)?)
}

pub fn cmd_start() -> Result {
    use configure::Configure;
    let cfg = ::config::Config::generate()?;

    debug!("Using config {:?}", cfg);
    let daemon = Daemon::new(cfg);

    info!("daemon ready");

    daemon.run()
}

pub fn cmd_stop() -> Result { self::send(Command::Stop) }

pub fn cmd_restart() -> Result { unimplemented!() }
