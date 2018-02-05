use std::path::PathBuf;

#[derive(Debug, Deserialize, Configure)]
#[serde(default)]
pub struct Config {
    pub socket: PathBuf,
    pub cache: PathBuf,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            socket: "/tmp/subs.sock".into(),
            cache: concat!(env!("HOME"), ".cache/subs").into(),
            url: "http://demo.subsonic.org".into(),
            username: "guest3".into(),
            password: "guest".into(),
            // ..Default::default()
        }
    }
}
