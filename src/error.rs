use serde_json;
use std::{io, result};
use sunk;

pub type Result = result::Result<(), Error>;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Subsonic error: {}", _0)] Subsonic(#[cause] sunk::Error),
    #[fail(display = "Unable to generate config: {}", _0)]
    Config(#[cause] config::ConfigError),
    #[fail(display = "{}", _0)] Other(&'static str),
    #[fail(display = "")] ExplicitStop,
    #[fail(display = "IO error: {}", _0)] Io(#[cause] io::Error),
    #[fail(display = "Serialisation error: {}", _0)]
    Serde(#[cause] serde_json::Error),
    #[fail(display = "Error from daemon: {}", _0)] Response(String),
}

macro_rules! box_err {
    ($err:ty, $to:ident) => {
        impl From<$err> for Error {
            fn from(err: $err) -> Error {
                self::Error::$to(err)
            }
        }
    };
}

box_err!(sunk::Error, Subsonic);
box_err!(config::ConfigError, Config);
box_err!(io::Error, Io);
box_err!(serde_json::Error, Serde);

impl From<&'static str> for Error {
    fn from(s: &'static str) -> Error { self::Error::Other(s) }
}
