use crossbeam_channel::*;
use rodio::{Decoder, Sink};
use serde_json;
use std::io::{BufReader, Cursor, Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use sunk::{Client, Hls, HlsPlaylist, Streamable};
use sunk::song::Song;
use unix_socket::{UnixListener, UnixStream};
use gst;
use gst::prelude::*;

use error::{Error, Result};
use queue::Queue;

#[derive(Serialize, Deserialize)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Stop,
    Add(usize),
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
                    debug!("received stream: {:?}", s);

                    let mut buf = String::new();
                    s.read_to_string(&mut buf)?;
                    let cmd = serde_json::from_str::<Command>(&buf)?;

                    match self.run_cmd(cmd) {
                        Err(Error::ExplicitStop) => {
                            info!("stop signal received.");
                            info!("stopping server.");
                            break
                        }
                        Err(e) => {
                            error!("Encountered error: {}", e);
                            return Err(e)
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

struct Player {
    daemon_recv: Receiver<Command>,
    daemon_send: Sender<Command>,
    client: Arc<Mutex<Client>>,
    sink: Sink,
    queue: Queue,
    curr_song: Option<HlsPlaylist>,
    curr_pos: usize,
    curr_pipe: Option<gst::Element>,
}

impl Player {
    fn new(
        daemon_recv: Receiver<Command>,
        daemon_send: Sender<Command>,
        client: Arc<Mutex<Client>>,
    ) -> Player {
        let sink = Sink::new(&::rodio::default_endpoint()
            .expect("unable to get default endpoint"));

        Player {
            daemon_recv,
            daemon_send,
            client,
            sink,
            queue: Queue::new(),
            curr_song: None,
            curr_pos: 0,
            curr_pipe: None,
        }
    }

    fn run(&mut self) {
        gst::init().expect("unable to initialise gstreamer");

        /// Fetches the next song in the playlist and sets it as the current
        /// song.
        fn prep_song(p: &mut Player) -> Result {
            if let Some(song) = p.queue.next() {
                let cli = &*p.client.lock().expect("unable to lock client");
                let s = Song::get(&cli, song as u64)?;
                info!("song: {:?}", s);
                p.curr_song = Some(s.hls(&cli, &[320])?);
                p.curr_pos = 0;
            }
            Ok(())
        };

        /// Fetches the next segment of the current song and appends it to the
        /// sink. Returns true if the song finishes with the new segment, and
        /// the length of the segment. If there's no next segment, returns false
        /// and a high duration.
        fn next_segment(p: &mut Player) -> (bool, u64) {
            if let Some(song) = p.curr_song.as_ref() {
                let cli = &*p.client.lock().expect("unable to lock client");
                let _seg = &song[p.curr_pos];
                let seg = cli.hls_bytes(_seg).expect("unable to fetch bytes");
                let dec =
                    Decoder::new(BufReader::new(Cursor::new(seg)))
                    .expect("unable to create decoder");
                p.sink.append(dec);
                p.curr_pos += 1;
                return (p.curr_pos == song.len() - 1, _seg.inc as u64)
            } else {
                return (false, 999)
            }
        };

        self.queue.append(1887);
        self.queue.append(1888);
        // Time before next segment has to be appended.
        // let mut sink_dur = (if self.queue.is_empty() {999} else {0}, 0);
        let mut sink_dur = if self.queue.is_empty() {999} else {0};
        loop {
            match self.daemon_recv.recv_timeout(Duration::from_secs(sink_dur)) {
                Ok(cmd) => match cmd {
                    Command::Add(song) => {
                        debug!("adding song {}", song);
                        self.queue.append(song);
                    }
                    _ => (),
                },
                Err(_) => {
                    if let Some(n) = self.queue.next() {
                        let cli = &*self.client.lock().expect("unable to lock client");
                        let song = Song::get(&cli, n as u64).unwrap();

                        let url = song.stream_url(&cli).unwrap();
                        let pipe = gst::parse_launch(&format!("playbin uri={}", url)).unwrap();
                        pipe.set_state(gst::State::Playing);
                        self.curr_pipe = Some(pipe);
                        sink_dur = song.duration.unwrap() as u64;
                    }

                    // The aim is to be playing one segment while prepping the
                    // next. The first one has to add two segments, and set the
                    // timeout to the first segment. When that pops, the player
                    // will be fetching the third segment while the second
                    // plays.
                    //
                    // When the timer pops to play the last segment of the
                    // current song, it has to fetch the first segment of the
                    // next song.
                    // match (self.curr_song.is_some(), self.queue.has_next()) {
                    //     // Currently playing, has next
                    //     (true, true) => {
                    //         if self.curr_pos - 1 ==
                    //             self.curr_song.as_ref().unwrap().len()
                    //         {
                    //             // Need next song
                    //             prep_song(self).unwrap();
                    //             let (_, inc) = next_segment(self);
                    //             sink_dur = (sink_dur.1, inc);
                    //         } else {
                    //             // Get next part of current song
                    //             let (_, inc) = next_segment(self);
                    //             sink_dur = (sink_dur.1, inc);
                    //         }
                    //     },
                    //     // Currently playing, doesn't have next
                    //     (true, false) => {
                    //         sink_dur = (sink_dur.1, 999);
                    //     },
                    //     // Nothing playing, has next; fetch two
                    //     (false, true) => {
                    //         prep_song(self).unwrap();
                    //         let (_, len1) = next_segment(self);
                    //         let (_, len2) = next_segment(self);
                    //         sink_dur = (len1, len2);
                    //     },
                    //     // Nothing playing, doesn't have next
                    //     (false, false) => {},
                    // }
                    // if self.curr_song.is_none() && self.queue.has_next() {
                    //     // not playing, has a queue
                    //     debug!("fetching next song");
                    //     prep_song(self);
                    // } else if self.curr_song.is_some() {
                    //     if self.queue.has_next() {
                    //         // currently playing, has a queue
                    //         if self.curr_song.as_ref().unwrap().len()
                    //             == self.curr_pos - 1
                    //         {
                    //             // on last segment, fetch next song
                    //             let cli = &*self.client
                    //                 .lock()
                    //                 .expect("unable to lock client");
                    //             let song = Song::get(
                    //                 &cli,
                    //                 self.queue.next().unwrap() as u64,
                    //             ).expect("error fetching song")
                    //                 .hls(&cli, &[320])
                    //                 .expect("error fetching hls");

                    //             let bytes = cli.hls_bytes(&song[0])
                    //                 .expect("error fetching segment");
                    //             let dec = Decoder::new(BufReader::new(
                    //                 Cursor::new(bytes),
                    //             )).expect("unable to decode bytes");
                    //             self.sink.append(dec);
                    //             self.curr_pos = 1;
                    //             sink_dur = song[0].inc as u64;
                    //             self.curr_song = Some(song);
                    //         }
                    //     } else {
                    //         // currently playing, nothing left in queue
                    //         sink_dur = 999;
                    //     }
                    // }
                    // debug!("checking sink");
                    // if self.sink.empty() {
                    //     debug!("checking next song");
                    //     // push the next item in the queue to the sink
                    //     let cli = &*self.client
                    //         .lock()
                    //         .expect("unable to lock client");
                    //     if let Some(idx) = self.queue.next() {
                    //         debug!("pushing song {}", idx);
                    //         let song = Song::get(&cli, idx as u64)
                    //             .expect("error fetching song");

                    //         if self.curr_song.is_none() {
                    //             self.curr_song = song.hls(&cli, &[320])
                    //                 .expect("error fetching HLS");
                    //             self.curr_pos = 0;
                    //         }
                    //         let bytes = BufReader::new(Cursor::new(
                    //             song.stream(&cli)
                    //                 .expect("error fetching bytes"),
                    //         ));
                    //         self.sink.append(
                    //             Decoder::new(bytes)
                    //                 .expect("unable to decode bytes"),
                    //         );
                    //     } else {
                    //         continue
                    //     }
                    // }
                }
            }
        }
    }
}

pub fn send(cmd: Command) -> Result {
    let cfg = ::config()?;
    let mut stream = UnixStream::connect(cfg.socket)?;

    stream.write_all(serde_json::to_string(&cmd)?.as_bytes())?;

    Ok(())
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
