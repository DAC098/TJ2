use std::collections::HashMap;
use std::default::Default;
use std::io::Read;
use std::net::{SocketAddr, IpAddr, Ipv6Addr};
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, ValueEnum};
use serde::Deserialize;

use crate::error::{self, Context};
use crate::path::normalize_from;

pub mod meta;

use meta::{
    TryDefault,
    SrcFile,
    DotPath,
    Quote,
    get_cwd,
    check_path,
    sanitize_url_key,
};

#[derive(Debug, Clone, ValueEnum)]
pub enum Verbosity {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Parser)]
pub struct CliArgs {
    pub config_path: PathBuf,

    #[arg(short = 'V', long)]
    pub verbosity: Option<Verbosity>
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
    assets: Option<AssetsShape>,
    templates: Option<TemplatesShape>,
}

#[derive(Debug)]
pub struct Settings {
    pub data: PathBuf,
    pub thread_pool: usize,
    pub blocking_pool: usize,
    pub listeners: Vec<Listener>,
    pub assets: Assets,
    pub templates: Templates,
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

        if let Some(assets) = settings.assets {
            self.assets.merge(src, dot.push(&"assets"), assets)?;
        }

        if let Some(templates) = settings.templates {
            self.templates.merge(src, dot.push(&"templates"), templates)?;
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
            assets: Assets::default(),
            templates: Templates::try_default()?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ListenerShape {
    addr: String,

    #[cfg(feature = "rustls")]
    tls: Option<tls::TlsShape>,
}

#[derive(Debug)]
pub struct Listener {
    pub addr: SocketAddr,

    #[cfg(feature = "rustls")]
    pub tls: Option<tls::Tls>,
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

        #[cfg(feature = "rustls")] {
            if let Some(tls) = listener.tls {
                let mut base = tls::Tls::default();

                base.merge(src, dot.push(&"tls"), tls)?;

                self.tls = Some(base);
            }
        }

        Ok(())
    }
}

impl Default for Listener {
    fn default() -> Self {
        Listener {
            addr: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8080),
            #[cfg(feature = "rustls")]
            tls: None,
        }
    }
}

#[cfg(feature = "rustls")]
pub mod tls {
    use std::path::PathBuf;

    use serde::Deserialize;

    use crate::error;
    use super::meta::{SrcFile, DotPath, check_path};

    #[derive(Debug, Deserialize)]
    pub struct TlsShape {
        key: PathBuf,
        cert: PathBuf,
    }

    #[derive(Debug, Default)]
    pub struct Tls {
        pub key: PathBuf,
        pub cert: PathBuf,
    }

    impl Tls {
        pub(super) fn merge(&mut self, src: &SrcFile<'_>, dot: DotPath<'_>, tls: TlsShape) -> Result<(), error::Error> {
            self.key = src.normalize(tls.key);
            self.cert = src.normalize(tls.cert);

            check_path(&self.key, src, dot.push(&"key"), true)?;
            check_path(&self.cert, src, dot.push(&"cert"), true)?;

            Ok(())
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AssetsShape {
    files: Option<HashMap<String, PathBuf>>,
    directories: Option<HashMap<String, PathBuf>>,
}

#[derive(Debug)]
pub struct Assets {
    pub files: HashMap<String, PathBuf>,
    pub directories: HashMap<String, PathBuf>,
}

impl Assets {
    fn merge(&mut self, src: &SrcFile<'_>, dot: DotPath<'_>, assets: AssetsShape) -> Result<(), error::Error> {
        if let Some(files) = assets.files {
            let files_dot = dot.push(&"files");

            for (url_key, path) in files {
                let key_quote = Quote(&url_key);
                let key = sanitize_url_key(&url_key, src, files_dot.push(&key_quote))?;

                let normalized = src.normalize(path);

                check_path(&normalized, src, files_dot.push(&key_quote), false)?;

                if let Some(found) = self.files.get_mut(&key) {
                    *found = normalized;
                } else {
                    self.files.insert(key, normalized);
                }
            }
        }

        if let Some(directories) = assets.directories {
            let dir_dot = dot.push(&"directories");

            for (url_key, path) in directories {
                let key_quote = Quote(&url_key);
                let key = sanitize_url_key(&url_key, src, dir_dot.push(&key_quote))?;

                let normalized = src.normalize(path);

                check_path(&normalized, src, dir_dot.push(&key_quote), false)?;

                if let Some(found) = self.directories.get_mut(&key) {
                    *found = normalized;
                } else {
                    self.directories.insert(key, normalized);
                }
            }
        }

        Ok(())
    }
}

impl Default for Assets {
    fn default() -> Self {
        Assets {
            files: HashMap::new(),
            directories: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TemplatesShape {
    directory: Option<PathBuf>
}

#[derive(Debug)]
pub struct Templates {
    pub directory: PathBuf
}

impl Templates {
    fn merge(&mut self, src: &SrcFile<'_>, dot: DotPath<'_>, templates: TemplatesShape) -> Result<(), error::Error> {
        if let Some(directory) = templates.directory {
            self.directory = src.normalize(directory);

            check_path(&self.directory, src, dot.push(&"directory"), false)?;
        }

        Ok(())
    }
}

impl TryDefault for Templates {
    type Error = error::Error;

    fn try_default() -> Result<Self, Self::Error> {
        Ok(Templates {
            directory: get_cwd()?.join("templates")
        })
    }
}
