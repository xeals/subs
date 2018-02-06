extern crate clap;
#[macro_use]
extern crate configure;
extern crate crossbeam_channel;
extern crate ctrlc;
#[macro_use]
extern crate failure;
extern crate fern;
extern crate gstreamer as gst;
#[macro_use]
extern crate log;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate sunk;
extern crate unix_socket;

mod cli;
mod error;
mod subcmd;
mod config;
mod daemon;
mod queue;
mod player;

use structopt::StructOpt;

fn main() {
    use_default_config!();
    let app = cli::App::from_args();

    if let Err(err) = init_logging(app.verbosity) {
        println!("[ERROR] Logging initialisation failed: {}", err);
    }

    use cli::AppCommand::*;
    use cli::ListCommand;
    if let Err(err) = match app.cmd {
        Load { name } => subcmd::load(name),
        Pause => subcmd::pause(),
        Play => subcmd::play(),
        Toggle => subcmd::toggle(),
        Prev => subcmd::prev(),
        Next => subcmd::next(),
        Add { query } => subcmd::add(collapse(query)),
        AddNext { query } => subcmd::addnext(collapse(query)),
        Clear => subcmd::clear(),
        Search { .. } => subcmd::search(app.cmd),
        Status => subcmd::status(),
        Random { number } => subcmd::random(number),
        Daemon { cmd } => {
            use cli::DaemonCommand::*;
            match cmd {
                Start => daemon::cmd_start(),
                Stop => daemon::cmd_stop(),
                Restart => daemon::cmd_restart(),
            }
        }
        _ => unimplemented!(),
    } {
        error!("{}", err);
        ::std::process::exit(1);
    }
}

fn init_logging(v: u64) -> Result<(), ::log::SetLoggerError> {
    let mut base = fern::Dispatch::new().format(|out, message, record| {
        out.finish(format_args!(
            "[{}][{}] {}",
            record.level(),
            record.target(),
            message
        ))
    });

    base = match v {
        0 => base.level(::log::LevelFilter::Error),
        1 => base.level(::log::LevelFilter::Warn),
        2 => base.level(::log::LevelFilter::Info),
        3 | _ => base.level(::log::LevelFilter::Debug),
    };

    base.chain(std::io::stdout()).apply()
}

pub fn collapse<T>(v: Vec<T>) -> String where T: Into<String> {
    v.into_iter().fold(String::new(), |a, s| a + " " + &s.into())
}

pub fn config() -> Result<config::Config, error::Error> {
    use configure::Configure;
    let cfg = config::Config::generate()?;

    debug!("Using config: {:?}", cfg);

    if !cfg.socket.exists() {
        return Err("Socket file doesn't exist; did you try running `subs \
                    daemon start`?"
            .into())
    }

    macro_rules! chk {
        ($f:ident) => (if cfg.$f == config::Config::default().$f {
            warn!("`SUBS_{}` is the default; do you want this?", stringify!($f).to_uppercase())
        });
    }

    chk!(url);
    chk!(username);
    chk!(password);

    Ok(cfg)
}
