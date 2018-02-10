use crossbeam_channel::*;
use gst;
use gst::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sunk::{Client, Streamable};
use sunk::song::Song;

use daemon::Command;
use error::Error;
use queue::Queue;

pub struct Player {
    daemon_recv: Receiver<Command>,
    daemon_send: Sender<Command>,
    client: Arc<Mutex<Client>>,
    queue: Queue,
    pipe: Option<gst::Element>,
    playing: bool,
    song_dur: u64,
    song_rem: u64,
}

impl Player {
    pub fn new(
        daemon_recv: Receiver<Command>,
        daemon_send: Sender<Command>,
        client: Arc<Mutex<Client>>,
    ) -> Player {
        Player {
            daemon_recv,
            daemon_send,
            client,
            queue: Queue::new(),
            pipe: None,
            playing: false,
            song_dur: 0,
            song_rem: 0,
        }
    }

    fn run_cmd(&mut self, cmd: Command) -> &'static str {
        match cmd {
            Command::Add(song) => {
                debug!("adding song {}", song);
                self.queue.append(song as usize);
            }
            Command::AddMany(ns) => {
                debug!("adding random songs: {:?}", ns);
                self.queue.extend(ns.iter().map(|i| *i as usize));
            }
            Command::AddNext(song) => {
                debug!("adding song {} next", song);
                self.queue.insert_next(song as usize);
            }
            Command::Clear => {
                debug!("emptying queue");
                self.queue.clear();
            }
            Command::Next => {
                debug!("skipping");
                // self.queue.next();
                self.song_rem = 0;
            }
            Command::Prev => {
                debug!("rewinding");
                self.queue.prev2();
                self.song_rem = 0;
            }
            Command::Play => {
                debug!("playing");
                if self.queue.is_empty() {
                    return "continue"
                }

                if let Some(ref pipe) = self.pipe {
                    log(pipe.set_state(gst::State::Playing));
                } else {
                    self.song_rem = 0;
                }

                self.playing = true;
            }
            Command::Pause => {
                debug!("pausing");
                if self.queue.is_empty() {
                    return "continue"
                }

                if let Some(ref pipe) = self.pipe {
                    log(pipe.set_state(gst::State::Paused));
                    self.song_rem = self.song_dur - secs(pipe);
                    info!("song duration left: {}", self.song_rem);
                } else {
                    self.song_rem = 0;
                }

                self.playing = false;
            }
            Command::Toggle => {
                debug!("toggling");
                if let Some(ref pipe) = self.pipe {
                    let (status, state, _) =
                        pipe.get_state(gst::CLOCK_TIME_NONE);
                    if status == gst::StateChangeReturn::Success {
                        use gst::State;
                        log(match state {
                            State::Playing => pipe.set_state(State::Paused),
                            State::Paused => pipe.set_state(State::Playing),
                            _ => gst::StateChangeReturn::Success,
                        });
                        self.song_rem = self.song_dur - secs(pipe);
                        self.playing = !self.playing;
                        info!("song duration left: {}", self.song_rem);
                    }
                } else {
                    self.song_rem = 0;
                }
            }
            Command::StatusReq => {
                debug!("sending status");
                let status = self.status();
                self.daemon_send.send(Command::Status(status)).unwrap();
            }
            Command::Stop => {
                debug!("stopping");
                return "break"
            }
            _ => info!("dunno what happened, boss"),
        }
        ""
    }

    pub fn run(&mut self) {
        gst::init().expect("unable to initialise gstreamer");

        'main: loop {
            if self.playing {
                match self.daemon_recv
                    .recv_timeout(Duration::from_secs(self.song_rem))
                {
                    Ok(cmd) => match self.run_cmd(cmd) {
                        "break" => break 'main,
                        "continue" => continue 'main,
                        _ => (),
                    },
                    Err(_) => {
                        info!("trying to play next song");
                        if let Some(n) = {
                            if self.pipe.is_some() {
                                self.queue.next()
                            } else {
                                self.queue.current()
                            }
                        } {
                            info!("playing next song: {}", n);
                            if let Some(ref pipe) = self.pipe {
                                log(pipe.set_state(gst::State::Null));
                            }
                            let cli = &*self.client
                                .lock()
                                .expect("unable to lock client");
                            let song = Song::get(cli, n as u64).unwrap();

                            let url = song.stream_url(cli).unwrap();
                            let pipe = gst::parse_launch(&format!(
                                "playbin uri={}",
                                url
                            )).expect("unable to start pipe");
                            log(pipe.set_state(gst::State::Playing));
                            self.pipe = Some(pipe);
                            self.song_dur = match song.duration {
                                Some(d) => d as u64,
                                None => song.size / 192 / 124,
                            };
                            self.song_rem = self.song_dur;
                            info!("song duration left: {}", self.song_dur);
                        } else {
                            warn!("queue is empty, what happened?");
                            self.song_rem = 9999;
                        }
                    }
                }
            } else {
                if let Ok(cmd) = self.daemon_recv.recv() {
                    self.run_cmd(cmd);
                } else {
                    error!("dunno what happened, boss");
                }
            }
        }
    }

    fn status(&self) -> String {
        fn secs_to_minsec(secs: u64) -> String {
            format!("{}:{:02}", secs / 60, secs % 60)
        }

        if let Some(song) = self.queue.current() {
            let (status, prog) = if let Some(ref pipe) = self.pipe {
                let state = pipe.get_state(gst::CLOCK_TIME_NONE).1;
                let status = if self.playing { "playing" } else { "paused" };
                let prog = pipe.query_position::<gst::ClockTime>()
                    .expect("error getting clock time")
                    .seconds()
                    .expect("unable to cast clock to seconds");

                (status, prog)
            } else {
                ("paused", 0)
            };

            let cli = &*self.client.lock().expect("unable to lock client");
            let curr_song = Song::get(cli, song as u64).unwrap();
            let dur = curr_song.duration.unwrap();

            format!(
                "{art}{title}\n[{stat}]  #{n}/{size}  {prog}/{dur} ({per})",
                art = if let Some(a) = curr_song.artist {
                    a + " - "
                } else {
                    "".to_string()
                },
                title = curr_song.title,
                stat = status,
                n = self.queue.position() + 1,
                size = self.queue.len(),
                prog = secs_to_minsec(prog),
                dur = secs_to_minsec(dur),
                per = if prog > 0 {
                    format!("{:.0}%", (prog as f32 / dur as f32) * 100.)
                } else {
                    "0%".into()
                },
            )
        } else {
            "nothing to display".into()
        }
    }
}

fn secs(pipe: &gst::Element) -> u64 {
    pipe.query_position::<gst::ClockTime>()
        .expect("error getting clock time")
        .seconds()
        .expect("unable to cast clock to seconds")
}

fn log(s: gst::StateChangeReturn) {
    if s != gst::StateChangeReturn::Success {
        error!("unable to change state: {:?}", s)
    }
}
