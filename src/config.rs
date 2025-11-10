//! all data pertaining to loading configuration files needed for server
//! operation.

use std::collections::HashMap;
use std::default::Default;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io::Read;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Parser;
use serde::Deserialize;
use tracing_subscriber::filter::{Directive, LevelFilter};

use crate::error::{self, Context};
use crate::path::{metadata, normalize_from};

pub mod meta;

use meta::{check_path, get_cwd, sanitize_url_key, DotPath, Quote, SrcFile, TryDefault};

/// specifies the verbosity level of the tracing logs
///
/// the verbosity is stacked so if you specify a high verbosity it will show
/// the lower ones as well. specify `"warn"` and it will display `"error"` as
/// well and if you specify `"trace"` it will display all previous verbosities
/// as well.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verbosity {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<&Verbosity> for LevelFilter {
    fn from(given: &Verbosity) -> Self {
        match given {
            Verbosity::Off => LevelFilter::OFF,
            Verbosity::Error => LevelFilter::ERROR,
            Verbosity::Warn => LevelFilter::WARN,
            Verbosity::Info => LevelFilter::INFO,
            Verbosity::Debug => LevelFilter::DEBUG,
            Verbosity::Trace => LevelFilter::TRACE,
        }
    }
}

impl From<&Verbosity> for Directive {
    fn from(given: &Verbosity) -> Self {
        LevelFilter::from(given).into()
    }
}

impl Display for Verbosity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Off => f.write_str("off"),
            Self::Error => f.write_str("error"),
            Self::Warn => f.write_str("warn"),
            Self::Info => f.write_str("info"),
            Self::Debug => f.write_str("debug"),
            Self::Trace => f.write_str("trace"),
        }
    }
}

/// the list of command line arguments that the server accepts
#[derive(Debug, Parser)]
pub struct CliArgs {
    /// specifies the config file to use when starting the server
    pub config_path: PathBuf,

    /// attempts to generate test data for the server to use for testing
    /// purposes
    #[arg(long)]
    pub gen_test_data: bool,
}

/// optional formats to output logs as
#[derive(Debug, Clone, Deserialize)]
pub enum LogFormat {
    /// outputs all logs in valid JSON
    Json,
    /// a more human readable log output
    Pretty,
    /// compact log format
    Compact,
}

/// a stack struct used when creating the Config struct
#[derive(Debug)]
struct ConfigStack {
    shape: SettingsShape,
    path: PathBuf,
    preload: std::vec::IntoIter<PathBuf>,
}

/// the final server configuration created from the loaded config file
#[derive(Debug)]
pub struct Config {
    pub settings: Settings,
}

impl Config {
    /// attempts to create the Config struct from the provided command line
    /// arguments
    ///
    /// when parsing a config file, it can specify a list of other files to
    /// load before working on the current file. each file loaded can
    /// overwrite the settings of the other and each file can also specify
    /// a list of files to preload before the current file.
    pub fn from_args(args: &CliArgs) -> Result<Self, error::Error> {
        let resolved = normalize_from(get_cwd()?, args.config_path.clone());
        let mut shape = Self::load_file(&resolved)?;

        let mut settings = Settings::try_default()?;
        let dot = DotPath::new(&"settings");

        let preload = shape.preload.take().unwrap_or_default();
        let mut stack: Vec<ConfigStack> = Vec::new();
        stack.push(ConfigStack {
            shape,
            path: resolved,
            preload: preload.into_iter(),
        });

        tracing::debug!("initial stack: {stack:#?}");

        while let Some(ConfigStack {
            shape,
            path,
            mut preload,
        }) = stack.pop()
        {
            if let Some(next_path) = preload.next() {
                let path_parent = path.parent().context(format!(
                    "failed to retrieve parent directory of path: \"{}\"",
                    path.display()
                ))?;

                let next_resolved = normalize_from(path_parent, next_path);
                let mut next_shape = Self::load_file(&next_resolved)?;
                let next_preload = next_shape.preload.take().unwrap_or_default();

                stack.push(ConfigStack {
                    shape,
                    path,
                    preload,
                });
                stack.push(ConfigStack {
                    shape: next_shape,
                    path: next_resolved,
                    preload: next_preload.into_iter(),
                });

                tracing::debug!("stack: {stack:#?}");

                continue;
            }

            let src = SrcFile::new(&path)?;

            tracing::debug!("merging settings file: {src}");

            settings.merge(&src, dot.clone(), shape)?;

            tracing::debug!("settings: {settings:#?}");
        }

        let data_meta = metadata(&settings.data)
            .context("failed to retrieve metadata for settings.data")?
            .context(format!(
                "settings.data was not found: {}",
                settings.data.display()
            ))?;

        if !data_meta.is_dir() {
            return Err(error::Error::context("settings.data is not a directory"));
        }

        let storage_meta = metadata(&settings.storage)
            .context("failed to retrieve metadata for settings.storage")?
            .context(format!(
                "settings.storage was not found: {}",
                settings.storage.display()
            ))?;

        if !storage_meta.is_dir() {
            return Err(error::Error::context("settings.storage is not a directory"));
        }

        if settings.listeners.is_empty() {
            return Err(error::Error::context(
                "no server listeners have been specified in config files",
            ));
        }

        Ok(Config { settings })
    }

    /// attempts to load a specified config file
    ///
    /// is capable of parsing JSON, YAML, and TOML files
    fn load_file(path: &PathBuf) -> Result<SettingsShape, error::Error> {
        let ext = path.extension().context(format!(
            "failed to retrieve the file extension from the config specified: \"{}\"",
            path.display()
        ))?;

        let ext = ext.to_ascii_lowercase();
        let mut contents = String::new();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .open(path)
            .context(format!(
                "failed to open config file: \"{}\"",
                path.display()
            ))?;

        file.read_to_string(&mut contents).context(format!(
            "failed to read contents of config file: \"{}\"",
            path.display()
        ))?;

        if ext.eq("json") {
            serde_json::from_str(&contents).context(format!(
                "failed to parse json config file: \"{}\"",
                path.display()
            ))
        } else if ext.eq("yaml") || ext.eq("yml") {
            serde_yml::from_str(&contents).context(format!(
                "failed to parse yaml config file: \"{}\"",
                path.display()
            ))
        } else if ext.eq("toml") {
            toml::from_str(&contents).context(format!(
                "failed to parse toml config file: \"{}\"",
                path.display()
            ))
        } else {
            Err(error::Error::context(format!(
                "unknown type of config file: \"{}\"",
                path.display()
            )))
        }
    }
}

/// the structure of a config file that can be loaded
#[derive(Debug, Deserialize)]
pub struct SettingsShape {
    preload: Option<Vec<PathBuf>>,
    data: Option<PathBuf>,
    storage: Option<PathBuf>,
    thread_pool: Option<usize>,
    blocking_pool: Option<usize>,
    logging: Option<LoggingShape>,
    listeners: Option<Vec<ListenerShape>>,
    assets: Option<AssetsShape>,
    templates: Option<TemplatesShape>,
    db: Option<DbShape>,
}

/// the root settings that are avaible for the server to use
#[derive(Debug)]
pub struct Settings {
    /// specifies the directory for the server to store information that is
    /// needed during operation
    ///
    /// defaults to "{CWD}/data"
    pub data: PathBuf,

    /// specifies the directory for the server to store user information that
    /// is created during operation
    ///
    /// defaults to "{CWD}/storage"
    pub storage: PathBuf,

    /// the number of asynchronous threads that tokio will use for the thread
    /// pool.
    ///
    /// defaults to 1
    pub thread_pool: usize,

    /// the number of blocking threads that tokio will use for synchronous
    /// operations.
    ///
    /// defaults to 1
    pub blocking_pool: usize,

    /// the available options for logging information for the server.
    pub logging: Option<Logging>,

    /// the list of available listeners for the server to use
    pub listeners: Vec<Listener>,

    /// the list of available public assets for the server to respond with
    pub assets: Assets,

    /// the available options for the template rendering system
    pub templates: Templates,

    /// configuration information for connecting to the database
    pub db: Db,
}

impl Settings {
    /// merges the given SettingsShape into the final Settings struct
    fn merge(
        &mut self,
        src: &SrcFile<'_>,
        dot: DotPath<'_>,
        settings: SettingsShape,
    ) -> Result<(), error::Error> {
        if let Some(data) = settings.data {
            self.data = src.normalize(data);

            check_path(&self.data, src, dot.push(&"data"), false)?;
        }

        if let Some(storage) = settings.storage {
            self.storage = src.normalize(storage);

            check_path(&self.storage, src, dot.push(&"data"), false)?;
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

        if let Some(logging) = settings.logging {
            if let Some(curr) = &mut self.logging {
                curr.merge(src, dot.push(&"logging"), logging)?;
            } else {
                let mut default = Logging::default();

                default.merge(src, dot.push(&"logging"), logging)?;

                self.logging = Some(default);
            }
        }

        if let Some(listeners) = settings.listeners {
            self.listeners = Vec::with_capacity(listeners.len());

            for listener in listeners {
                let mut default = Listener::default();
                default.merge(src, dot.push(&"listeners"), listener)?;

                self.listeners.push(default);
            }
        }

        if let Some(assets) = settings.assets {
            self.assets.merge(src, dot.push(&"assets"), assets)?;
        }

        if let Some(templates) = settings.templates {
            self.templates
                .merge(src, dot.push(&"templates"), templates)?;
        }

        if let Some(db) = settings.db {
            self.db.merge(src, dot.push(&"db"), db)?;
        }

        Ok(())
    }
}

impl TryDefault for Settings {
    type Error = error::Error;

    fn try_default() -> Result<Self, Self::Error> {
        Ok(Settings {
            data: get_cwd()?.join("data"),
            storage: get_cwd()?.join("storage"),
            thread_pool: 1,
            blocking_pool: 1,
            logging: None,
            listeners: Vec::new(),
            assets: Assets::default(),
            templates: Templates::try_default()?,
            db: Db::default(),
        })
    }
}

/// the structure of logging loaded from a config file
#[derive(Debug, Deserialize)]
pub struct LoggingShape {
    verbosity: Option<Verbosity>,
    format: Option<LogFormat>,
    directives: Option<HashMap<String, Verbosity>>,
    output: Option<LoggingOutputShape>,
}

/// the structure of loggout output loaded from a config file
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LoggingOutputShape {
    Stdio,
    File {
        directory: Option<PathBuf>,
        rotation: Option<LoggingRotation>,
        max_files: Option<usize>,
        prefix: Option<String>,
    },
}

/// specifies the the kinds of rotation available for file logging output.
///
/// it maps to [`tracing_appender::rolling::Rotation`]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoggingRotation {
    /// rotates logs every minute
    Minutely,
    /// rotates logs every hour
    Hourly,
    /// rotates logs every day
    Daily,
    /// consolodates logs to a single file
    Never,
}

impl From<&LoggingRotation> for tracing_appender::rolling::Rotation {
    fn from(given: &LoggingRotation) -> Self {
        match given {
            LoggingRotation::Minutely => tracing_appender::rolling::Rotation::MINUTELY,
            LoggingRotation::Hourly => tracing_appender::rolling::Rotation::HOURLY,
            LoggingRotation::Daily => tracing_appender::rolling::Rotation::DAILY,
            LoggingRotation::Never => tracing_appender::rolling::Rotation::NEVER,
        }
    }
}

/// the list of options available for configuring logging for the server
///
/// logging is done using [`tracing`] and combines options from
/// [`tracing_subscriber`] and [`tracing_appender`]
#[derive(Debug)]
pub struct Logging {
    pub verbosity: Verbosity,
    pub format: Option<LogFormat>,
    pub directives: HashMap<String, Verbosity>,
    pub output: LoggingOutput,
}

/// the available options for log output
#[derive(Debug)]
pub enum LoggingOutput {
    /// logs will be sent to stdout or stderr
    Stdio,

    /// logs will be sent to a file under a specified directory
    File {
        /// the directory to store all log files generated by the server
        directory: PathBuf,

        /// the rotation time for new logs to be created
        ///
        /// defaults to [`LoggingRotation::Daily`]
        rotation: LoggingRotation,

        /// the max number of log files to keep
        ///
        /// once the max number is reached the oldest logs will be removed
        /// refer to [`tracing_appender::rolling::Builder::max_log_files`]
        /// for details on how it is calculated
        max_files: Option<usize>,

        /// the log file prefix to use
        ///
        /// defaults to `"rsf_server"`
        prefix: String,
    },
}

impl Logging {
    /// merges the given SettingsShape into the final Settings struct
    fn merge(
        &mut self,
        src: &SrcFile<'_>,
        dot: DotPath<'_>,
        logging: LoggingShape,
    ) -> Result<(), error::Error> {
        if let Some(verbosity) = logging.verbosity {
            self.verbosity = verbosity;
        }

        self.format = logging.format;

        if let Some(directives) = logging.directives {
            for (module, verbosity) in directives {
                self.directives.insert(module, verbosity);
            }
        }

        if let Some(output) = logging.output {
            // well this is a big jank
            let curr = std::mem::replace(&mut self.output, LoggingOutput::Stdio);

            self.output = curr.merge(src, dot.push(&"output"), output)?;
        }

        Ok(())
    }
}

impl LoggingOutput {
    fn merge(
        self,
        src: &SrcFile<'_>,
        dot: DotPath<'_>,
        output: LoggingOutputShape,
    ) -> Result<Self, error::Error> {
        Ok(match (self, output) {
            (LoggingOutput::Stdio, LoggingOutputShape::Stdio) => Self::Stdio,
            (
                LoggingOutput::Stdio,
                LoggingOutputShape::File {
                    directory,
                    rotation,
                    max_files,
                    prefix,
                },
            ) => {
                let mut directory = directory.unwrap_or(PathBuf::new());

                if !directory.as_os_str().is_empty() {
                    let norm = src.normalize(directory);

                    check_path(&norm, src, dot.push(&"directory"), false)?;

                    directory = norm;
                }

                Self::File {
                    directory,
                    rotation: rotation.unwrap_or(LoggingRotation::Daily),
                    max_files,
                    prefix: prefix.unwrap_or(String::from("rsf_server")),
                }
            }
            (LoggingOutput::File { .. }, LoggingOutputShape::Stdio) => Self::Stdio,
            (
                LoggingOutput::File {
                    mut directory,
                    mut rotation,
                    mut max_files,
                    mut prefix,
                },
                LoggingOutputShape::File {
                    directory: shape_dir,
                    rotation: shape_rot,
                    max_files: shape_max,
                    prefix: shape_pfx,
                },
            ) => {
                if let Some(dir) = shape_dir {
                    directory = src.normalize(dir);

                    check_path(&directory, src, dot.push(&"directory"), false)?;
                }

                if let Some(rot) = shape_rot {
                    rotation = rot;
                }

                if let Some(max) = shape_max {
                    max_files = Some(max);
                }

                if let Some(pfx) = shape_pfx {
                    prefix = pfx;
                }

                Self::File {
                    directory,
                    rotation,
                    max_files,
                    prefix,
                }
            }
        })
    }
}

impl Default for Logging {
    fn default() -> Self {
        Self {
            verbosity: Verbosity::Off,
            format: None,
            directives: HashMap::new(),
            output: LoggingOutput::Stdio,
        }
    }
}

/// the structure of a listener loaded from a config file
#[derive(Debug, Deserialize)]
pub struct ListenerShape {
    addr: String,

    #[cfg(feature = "rustls")]
    tls: Option<tls::TlsShape>,
}

/// the final structure of a listener
#[derive(Debug)]
pub struct Listener {
    /// the ipv4/ipv6 ip and port for the server to listen on
    pub addr: SocketAddr,

    /// additional tls information for the specific listener to use
    #[cfg(feature = "rustls")]
    pub tls: Option<tls::Tls>,
}

impl Listener {
    /// merges the given ListenerShape into the final Listener struct
    fn merge(
        &mut self,
        src: &SrcFile<'_>,
        dot: DotPath<'_>,
        listener: ListenerShape,
    ) -> Result<(), error::Error> {
        self.addr = match SocketAddr::from_str(&listener.addr) {
            Ok(valid) => valid,
            Err(_) => match IpAddr::from_str(&listener.addr) {
                Ok(valid) => SocketAddr::from((valid, 8080)),
                Err(_) => {
                    return Err(error::Error::context(format!(
                        "{dot}.addr invalid: \"{}\" file: {src}",
                        listener.addr
                    )))
                }
            },
        };

        #[cfg(feature = "rustls")]
        {
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

    use super::meta::{check_path, DotPath, SrcFile};
    use crate::error;

    /// the structure of a tls listener from a config file
    #[derive(Debug, Deserialize)]
    pub struct TlsShape {
        key: PathBuf,
        cert: PathBuf,
    }

    /// the settings available to create a tls listener
    #[derive(Debug, Default)]
    pub struct Tls {
        /// the specified path of the private key to use
        pub key: PathBuf,

        /// the speicifed path of the certificate to use
        pub cert: PathBuf,
    }

    impl Tls {
        /// merges a given TlsShape into a Tls structure
        pub(super) fn merge(
            &mut self,
            src: &SrcFile<'_>,
            dot: DotPath<'_>,
            tls: TlsShape,
        ) -> Result<(), error::Error> {
            self.key = src.normalize(tls.key);
            self.cert = src.normalize(tls.cert);

            check_path(&self.key, src, dot.push(&"key"), true)?;
            check_path(&self.cert, src, dot.push(&"cert"), true)?;

            Ok(())
        }
    }
}

/// the structure of an assets config
#[derive(Debug, Deserialize)]
pub struct AssetsShape {
    files: Option<HashMap<String, PathBuf>>,
    directories: Option<HashMap<String, PathBuf>>,
}

/// lists the available files and directories that are publicly available for
/// the server to respond with.
#[derive(Debug, Default)]
pub struct Assets {
    /// lists individual files that the server will respond with when directly
    /// requested.
    ///
    /// when loading config files, the provided files will be merged with the
    /// known list. if a file is specified in more than one config then the
    /// last entry will be used.
    pub files: HashMap<String, PathBuf>,

    /// lists directories that the server will do lookups in when a file is
    /// requested but not found in the files map.
    ///
    /// similar to the files map in how config files are loaded
    pub directories: HashMap<String, PathBuf>,
}

impl Assets {
    /// merges a given AssetsShape into an Assets structure
    fn merge(
        &mut self,
        src: &SrcFile<'_>,
        dot: DotPath<'_>,
        assets: AssetsShape,
    ) -> Result<(), error::Error> {
        if let Some(files) = assets.files {
            let files_dot = dot.push(&"files");

            for (url_key, path) in files {
                let key_quote = Quote(&url_key);
                let key = sanitize_url_key(&url_key, src, files_dot.push(&key_quote))?;

                let normalized = src.normalize(path);

                check_path(&normalized, src, files_dot.push(&key_quote), true)?;

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

/// the structure of a templates config
#[derive(Debug, Deserialize)]
pub struct TemplatesShape {
    directory: Option<PathBuf>,
}

/// the list of available options when configuring the templates for a server
/// to use.
#[derive(Debug)]
pub struct Templates {
    /// the directory that contains all templates for the server to load
    ///
    /// defaults to "{CWD}/templates"
    pub directory: PathBuf,
}

impl Templates {
    /// merges a given TemplatesShape into a Templates structure
    fn merge(
        &mut self,
        src: &SrcFile<'_>,
        dot: DotPath<'_>,
        templates: TemplatesShape,
    ) -> Result<(), error::Error> {
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
            directory: get_cwd()?.join("templates"),
        })
    }
}

/// the structure of a db config
#[derive(Debug, Deserialize)]
pub struct DbShape {
    user: Option<String>,
    password: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    dbname: Option<String>,
}

/// the available options when connecting to the database
#[derive(Debug)]
pub struct Db {
    /// the user for connecting to the database
    ///
    /// defaults to "postgres"
    pub user: String,

    /// the optional password for the user
    ///
    /// defaults to None
    pub password: Option<String>,

    /// the hostname of the database
    ///
    /// defaults to "localhost"
    pub host: String,

    /// the port the database is listening on
    ///
    /// defaults to 5432
    pub port: u16,

    /// the name of the database to connect to
    ///
    /// defaults to "tj2"
    pub dbname: String,
}

impl Db {
    /// merges a given DbShape into a Db structure
    fn merge(
        &mut self,
        _src: &SrcFile<'_>,
        _dot: DotPath<'_>,
        db: DbShape,
    ) -> Result<(), error::Error> {
        if let Some(user) = db.user {
            self.user = user;
        }

        if let Some(password) = db.password {
            self.password = Some(password);
        }

        if let Some(host) = db.host {
            self.host = host;
        }

        if let Some(port) = db.port {
            self.port = port;
        }

        if let Some(dbname) = db.dbname {
            self.dbname = dbname;
        }

        Ok(())
    }
}

impl Default for Db {
    fn default() -> Self {
        Self {
            user: "postgres".to_owned(),
            password: None,
            host: "localhost".to_owned(),
            port: 5432,
            dbname: "tj2".to_owned(),
        }
    }
}
