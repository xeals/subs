use crossbeam_channel::*;
use gst;
use gst::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sunk::{Client, Streamable};
use sunk::song::Song;

use error::Error;
use daemon::Command;
use queue::Queue;

pub struct Player {
    daemon_recv: Receiver<Command>,
    daemon_send: Sender<Command>,
    client: Arc<Mutex<Client>>,
    queue: Queue,
    pipe: Option<gst::Element>,
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
        }
    }

    pub fn run(&mut self) {
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

        gst::init().expect("unable to initialise gstreamer");

        let mut song_dur = 0;
        let mut song_rem = 9999;
        loop {
            match self.daemon_recv.recv_timeout(Duration::from_secs(song_rem)) {
                Ok(cmd) => match cmd {
                    Command::Add(song) => {
                        debug!("adding song {}", song);
                        self.queue.append(song as usize);
                    }
                    Command::Play => {
                        debug!("playing");
                        if self.queue.is_empty() {
                            continue
                        }

                        if let Some(ref pipe) = self.pipe {
                            log(pipe.set_state(gst::State::Playing));
                        } else {
                            song_rem = 0;
                        }
                    }
                    Command::Pause => {
                        debug!("pausing");
                        if self.queue.is_empty() {
                            continue
                        }

                        if let Some(ref pipe) = self.pipe {
                            log(pipe.set_state(gst::State::Paused));
                            song_rem = song_dur - secs(pipe);
                            info!("left: {}", song_rem);
                        } else {
                            song_rem = 0;
                        }
                    }
                    Command::Toggle => {
                        debug!("toggling");
                        if let Some(ref pipe) = self.pipe {
                            let (status, state, _) =
                                pipe.get_state(gst::CLOCK_TIME_NONE);
                            if status == gst::StateChangeReturn::Success {
                                use gst::State;
                                log(match state {
                                    State::Playing => {
                                        pipe.set_state(State::Paused)
                                    }
                                    State::Paused => {
                                        pipe.set_state(State::Playing)
                                    }
                                    _ => gst::StateChangeReturn::Success,
                                });
                                song_rem = song_dur - secs(pipe);
                                info!("left: {}", song_rem);
                            }
                        }
                    }
                    Command::StatusReq => {
                        debug!("sending status");
                        let status = self.status();
                        self.daemon_send.send(Command::Status(status));
                    }
                    Command::Stop => {
                        debug!("stopping");
                        break
                    }
                    _ => info!("dunno what happened, boss"),
                },
                Err(_) => {
                    if let Some(n) = self.queue.next() {
                        info!("playing next song: {}", n);
                        let cli = &*self.client
                            .lock()
                            .expect("unable to lock client");
                        let song = Song::get(cli, n as u64).unwrap();

                        let url = song.stream_url(cli).unwrap();
                        let pipe =
                            gst::parse_launch(&format!("playbin uri={}", url))
                                .expect("unable to start pipe");
                        log(pipe.set_state(gst::State::Playing));
                        self.pipe = Some(pipe);
                        song_dur = song.duration.unwrap() as u64 - 2;
                        song_rem = song_dur;
                        info!("left: {}", song_dur);
                    } else {
                        info!("queue is empty, what happened?");
                        song_rem = 999;
                    }
                }
            }
        }
    }

    fn status(&self) -> String {
        fn secs_to_minsec(secs: u64) -> String {
            if secs >= 60 {
                format!("{}:{}", secs / 60, secs % 60)
            } else {
                secs.to_string()
            }
        }

        if let Some(song) = self.queue.current() {
            let (status, prog) = if let Some(ref pipe) = self.pipe {
                let state = pipe.get_state(gst::CLOCK_TIME_NONE).1;
                let status = match state {
                    gst::State::Playing => "playing",
                    gst::State::Paused => "paused",
                    _ => "???"
                };
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
                n = self.queue.position(),
                size = self.queue.len(),
                prog = secs_to_minsec(prog),
                dur = secs_to_minsec(dur),
                per = if prog > 0 {
                    format!("{:.0}", dur / prog)
                } else {
                    "0".into()
                },
            )
        } else {
            "nothing to display".into()
        }
    }
}
