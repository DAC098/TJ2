use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

use crate::config::{Config, LogFormat, LoggingOutput};
use crate::error::{Context, Error};

/// initalizes the logging for the server
///
/// the server will be able to support both stdout logging and rotating log
/// files. the rotation time frame for files will be daily and will only be
/// used if the log_dir arg is specified from the command line.
pub fn init(config: &Config) -> Result<Option<WorkerGuard>, Error> {
    let Some(logging) = &config.settings.logging else {
        return Ok(None);
    };

    let mut filter = EnvFilter::from_default_env().add_directive((&logging.verbosity).into());

    for (key, verbosity) in logging.directives.iter() {
        let directive = format!("{key}={verbosity}");
        let msg = format!("invalid logging directive given: {directive}");

        tracing::debug!("adding directive: \"{directive}\"");

        filter = filter.add_directive(directive.parse().context(msg)?);
    }

    let log_builder = tracing_subscriber::fmt().with_env_filter(filter);

    match &logging.output {
        LoggingOutput::Stdio => {
            let result = if let Some(format) = &logging.format {
                match format {
                    LogFormat::Json => log_builder.json().try_init(),
                    LogFormat::Pretty => log_builder.pretty().try_init(),
                    LogFormat::Compact => log_builder.compact().try_init(),
                }
            } else {
                log_builder.try_init()
            };

            if let Err(err) = result {
                Err(Error::context_source(
                    "failed to initialize stdoubt logging",
                    err,
                ))
            } else {
                Ok(None)
            }
        }
        LoggingOutput::File {
            directory,
            rotation,
            max_files,
            prefix,
        } => {
            let mut builder = RollingFileAppender::builder()
                .rotation(rotation.into())
                .filename_prefix(prefix)
                .filename_suffix("log");

            if let Some(max) = max_files {
                builder = builder.max_log_files(*max);
            }

            let appender = builder
                .build(&directory)
                .context("failed to initialize rotating logs")?;

            let (non_blocking, guard) = tracing_appender::non_blocking(appender);

            let builder = log_builder.with_writer(non_blocking).with_ansi(false);

            let result = if let Some(format) = &logging.format {
                match format {
                    LogFormat::Json => builder.json().try_init(),
                    LogFormat::Pretty => builder.pretty().try_init(),
                    LogFormat::Compact => builder.compact().try_init(),
                }
            } else {
                builder.try_init()
            };

            if let Err(err) = result {
                Err(Error::context_source(
                    "failed to initialize rotating logs",
                    err,
                ))
            } else {
                Ok(Some(guard))
            }
        }
    }
}

/// creates a default stdio subscriber with a default directive of [`LevelFilter::ERROR`]
///
/// will also load from the env otherwise there will be no way of specifying
/// the verbosity without manually changing the code.
pub fn stdio_subscriber() -> impl tracing::Subscriber {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::ERROR.into())
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(filter).finish()
}
