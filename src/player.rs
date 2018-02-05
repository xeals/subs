use gst;
use gst::prelude::*;
use crossbeam_channel::*;
use std::time::{Duration};
use sunk::{Client, Streamable};
use sunk::song::Song;
use std::sync::{Arc, Mutex};

use daemon::Command;
use error::{Error, Result};
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
                        let song = Song::get(&cli, n as u64).unwrap();

                        let url = song.stream_url(&cli).unwrap();
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
}

