use std::path::PathBuf;
use std::default::Default;
use std::io::Read;
use std::net::{SocketAddr, IpAddr, Ipv6Addr};
use std::str::FromStr;

use clap::Parser;
use serde::Deserialize;

use crate::error::{self, Context};
use crate::path::normalize_from;

pub mod meta;

use meta::{TryDefault, SrcFile, DotPath, get_cwd, check_path};

#[derive(Debug, Parser)]
pub struct CliArgs {
    config_path: PathBuf
}

#[derive(Debug)]
pub struct Config {
    pub settings: Settings,
}

impl Config {
    pub fn from_args(args: &CliArgs) -> Result<Self, error::Error> {
        let resolved = normalize_from(get_cwd()?, args.config_path.clone());
        let shape = Self::load_file(&resolved)?;

        let mut settings = Settings::try_default()?;
        let src = SrcFile::new(&resolved)?;
        let dot = DotPath::new(&"settings");

        settings.merge(&src, dot.clone(), shape)?;

        check_path(&settings.data, &src, dot.push(&"data"), false)?;

        Ok(Config {
            settings
        })
    }

    fn load_file(path: &PathBuf) -> Result<SettingsShape, error::Error> {
        let ext = path.extension().context(format!(
            "failed to retrieve the file extension from the config specified: \"{}\"", path.display()
        ))?;

        let ext = ext.to_ascii_lowercase();
        let mut contents = String::new();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .open(path)
            .context(format!("failed to open config file: \"{}\"", path.display()))?;

        file.read_to_string(&mut contents)
            .context(format!("failed to read contents of config file: \"{}\"", path.display()))?;

        if ext.eq("json") {
            serde_json::from_str(&contents).context(format!(
                "failed to parse json config file: \"{}\"", path.display()
            ))
        } else if ext.eq("yaml") || ext.eq("yml") {
            serde_yml::from_str(&contents).context(format!(
                "failed to parse yaml config file: \"{}\"", path.display()
            ))
        } else if ext.eq("toml") {
            toml::from_str(&contents).context(format!(
                "failed to parse toml config file: \"{}\"", path.display()
            ))
        } else {
            Err(error::Error::context(format!(
                "unknown type of config file: \"{}\"", path.display()
            )))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SettingsShape {
    data: Option<PathBuf>,
    thread_pool: Option<usize>,
    blocking_pool: Option<usize>,
    listeners: Vec<ListenerShape>,
}

#[derive(Debug)]
pub struct Settings {
    pub data: PathBuf,
    pub thread_pool: usize,
    pub blocking_pool: usize,
    pub listeners: Vec<Listener>,
}

impl Settings {
    fn merge(&mut self, src: &SrcFile<'_>, dot: DotPath<'_>, settings: SettingsShape) -> Result<(), error::Error> {
        if let Some(data) = settings.data {
            self.data = src.normalize(data);

            check_path(&self.data, src, dot.push(&"data"), false)?;
        }

        if let Some(thread_pool) = settings.thread_pool {
            if thread_pool == 0 {
                return Err(error::Error::context(format!(
                    "{dot}.thread_pool amount is 0 in {src}"
                )));
            }

            self.thread_pool = thread_pool;
        }

        if let Some(blocking_pool) = settings.blocking_pool {
            if blocking_pool == 0 {
                return Err(error::Error::context(format!(
                    "{dot}.blocking_pool amount is 0 in {src}"
                )));
            }

            self.blocking_pool = blocking_pool;
        }

        let system_cpus = num_cpus::get();

        if self.thread_pool + self.blocking_pool > system_cpus {
            println!("WARNING: total number of threads exceeds the systems");
        }

        self.listeners = Vec::with_capacity(settings.listeners.len());

        for listener in settings.listeners {
            let mut default = Listener::default();
            default.merge(src, dot.push(&"listeners"), listener)?;

            self.listeners.push(default);
        }

        Ok(())
    }
}

impl TryDefault for Settings {
    type Error = error::Error;

    fn try_default() -> Result<Self, Self::Error> {
        Ok(Settings {
            data: get_cwd()?.join("data"),
            thread_pool: 1,
            blocking_pool: 1,
            listeners: vec![Listener::default()],
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ListenerShape {
    addr: String
}

#[derive(Debug)]
pub struct Listener {
    pub addr: SocketAddr
}

impl Listener {
    fn merge(&mut self, src: &SrcFile<'_>, dot: DotPath<'_>, listener: ListenerShape) -> Result<(), error::Error> {
        self.addr = match SocketAddr::from_str(&listener.addr) {
            Ok(valid) => valid,
            Err(_) => match IpAddr::from_str(&listener.addr) {
                Ok(valid) => SocketAddr::from((valid, 8080)),
                Err(_) => return Err(error::Error::context(format!(
                    "{dot}.addr invalid: \"{}\" file: {src}", listener.addr
                )))
            }
        };

        Ok(())
    }
}

impl Default for Listener {
    fn default() -> Self {
        Listener {
            addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8080)
        }
    }
}
