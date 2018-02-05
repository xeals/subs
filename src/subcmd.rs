use cli::AppCommand;
use daemon::{self, Command, Reply};
use error::{Error, Result};

pub fn play() -> Result { daemon::send(Command::Play) }
pub fn pause() -> Result { daemon::send(Command::Pause) }
pub fn toggle() -> Result { daemon::send(Command::Toggle) }
pub fn prev() -> Result { daemon::send(Command::Prev) }
pub fn next() -> Result { daemon::send(Command::Next) }

pub fn add(query: String) -> Result {
    if let Reply::Other(r) = daemon::send_recv(Command::AddSearch(query))? {
        if r.is_empty() {
            Ok(())
        } else {
            Err(Error::Response(r))
        }
    } else {
        unreachable!()
    }
}

pub fn load(name: String) -> Result { unimplemented!() }

pub fn search(args: AppCommand) -> Result {
    if let AppCommand::Search {
        query,
        only_artists,
        only_albums,
        only_songs,
    } = args
    {
        let resp =
            daemon::send_recv(Command::Search(query, false, false, true))?;
        if let Reply::Search {
            albums,
            artists,
            songs,
        } = resp
        {
            for s in songs {
                println!("{}", s);
            }
        }
    } else {
        unreachable!()
    }

    Ok(())
}

pub fn status() -> Result { unimplemented!() }
