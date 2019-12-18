use config::{Config, Environment, File};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Conf {
    pub socket: PathBuf,
    pub cache: PathBuf,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Conf {
    pub fn new() -> Result<Self, ::error::Error> {
        let mut conf = Config::try_from(&Conf::default())?;
        let config_path = dirs::config_dir()
            .expect("Unable to find a valid path for config, your platform might not be supported")
            .join("subs");
        conf.merge(File::with_name(config_path
            .to_str()
            .expect("Config directory path can't be interpreted")))?;
        conf.merge(Environment::with_prefix("subs"))?;
        let cfg: Self = conf.try_into()?;

        debug!("Using config: {:?}", cfg);

        if !cfg.socket.exists() {
            return Err("Socket file doesn't exist; did you try running `subs \
                        daemon start`?"
                .into())
        }

        macro_rules! chk {
            ($f:ident) => (if cfg.$f == Self::default().$f {
                warn!("`SUBS_{}` is the default; do you want this?", stringify!($f).to_uppercase())
            });
        }

        chk!(url);
        chk!(username);
        chk!(password);

        Ok(cfg)
    }
}

impl Default for Conf {
    fn default() -> Conf {
        let runtime = dirs::runtime_dir().unwrap_or("/tmp".into());
        Conf {
            socket: runtime.join("subs.sock"),
            cache: dirs::cache_dir()
                .expect("Unable to find a valid path for cache, your platform might not be supported")
                .join("subs"),
            url: "http://demo.subsonic.org".into(),
            username: "guest3".into(),
            password: "guest".into(),
        }
    }
}
