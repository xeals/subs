use cli::AppCommand;
use daemon::{self, Command, Reply};
use error::{Error, Result};

pub fn clear() -> Result { daemon::send(Command::Clear) }
pub fn next() -> Result { daemon::send(Command::Next) }
pub fn pause() -> Result { daemon::send(Command::Pause) }
pub fn play() -> Result { daemon::send(Command::Play) }
pub fn prev() -> Result { daemon::send(Command::Prev) }
pub fn random(n: usize) -> Result { daemon::send(Command::Random(n)) }
pub fn toggle() -> Result { daemon::send(Command::Toggle) }

pub fn add(query: String) -> Result {
    if let Reply::Other(r) = daemon::send_recv(Command::AddSearch(query))? {
        if !r.starts_with("Nothing") {
            println!("{}", r);
            Ok(())
        } else {
            Err(Error::Response(r))
        }
    } else {
        unreachable!()
    }
}

pub fn addnext(query: String) -> Result {
    if let Reply::Other(r) = daemon::send_recv(Command::AddNextSearch(query))? {
        if !r.starts_with("Nothing") {
            println!("{}", r);
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
        number,
    } = args
    {
        let resp = daemon::send_recv(Command::Search(
            ::collapse(query),
            only_artists,
            only_albums,
            !(only_artists || only_albums),
            number,
        ))?;

        if let Reply::Search {
            artists,
            albums,
            songs,
        } = resp
        {
            for item in if only_artists {
                artists
            } else if only_albums {
                albums
            } else {
                songs
            } {
                println!("{}", item);
            }
        }
    } else {
        unreachable!()
    }

    Ok(())
}

pub fn status() -> Result {
    if let Reply::Other(r) = daemon::send_recv(Command::StatusReq)? {
        println!("{}", r);
        Ok(())
    } else {
        unreachable!()
    }
}

pub fn ls_playlists() -> Result {
    unimplemented!()
}

pub fn ls_artists(number: usize) -> Result {
    unimplemented!()
}
