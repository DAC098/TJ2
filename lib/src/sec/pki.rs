use std::path::Path;

use chrono::{DateTime, Utc};
use crypto_box::{SecretKey, KEY_SIZE};
use rand::RngCore;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

#[derive(Debug)]
pub struct PrivateKey {
    created: DateTime<Utc>,
    secret: crypto_box::SecretKey
}

type BinaryFormat = (i64, [u8; KEY_SIZE]);

#[derive(Debug, thiserror::Error)]
pub enum PrivateKeyError {
    #[error("invalid timestamp")]
    InvalidTimestamp,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Rand(#[from] rand::Error),

    #[error(transparent)]
    Encode(#[from] bincode::error::EncodeError),
    #[error(transparent)]
    Decode(#[from] bincode::error::DecodeError),
}

impl PrivateKey {
    fn binary_config() -> bincode::config::Configuration {
        bincode::config::standard()
    }

    pub fn generate() -> Result<Self, PrivateKeyError> {
        let created = Utc::now();
        let mut bytes = [0u8; crypto_box::KEY_SIZE];

        rand::rngs::OsRng.try_fill_bytes(&mut bytes)?;

        Ok(Self {
            created,
            secret: SecretKey::from_bytes(bytes)
        })
    }

    pub fn from_bytes(given: &[u8]) -> Result<Self, PrivateKeyError> {
        let ((ts, bytes), _): (BinaryFormat, _) = bincode::decode_from_slice(given, Self::binary_config())?;

        let created = DateTime::from_timestamp(ts, 0)
            .ok_or(PrivateKeyError::InvalidTimestamp)?;
        let secret = SecretKey::from_bytes(bytes);

        Ok(Self {
            created,
            secret,
        })
    }

    pub async fn load<P>(path: P) -> Result<Self, PrivateKeyError>
    where
        P: AsRef<Path>
    {
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .await?;
        let mut contents = Vec::new();

        file.read_to_end(&mut contents).await?;

        Self::from_bytes(&contents)
    }

    pub fn created(&self) -> &DateTime<Utc> {
        &self.created
    }

    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }

    pub fn as_bytes(&self) -> Result<Vec<u8>, PrivateKeyError> {
        let ts = self.created.timestamp();
        let bytes = self.secret.to_bytes();

        Ok(bincode::encode_to_vec((ts, bytes), Self::binary_config())?)
    }

    pub async fn save<P>(&self, path: P, overwrite: bool) -> Result<(), PrivateKeyError>
    where
        P: AsRef<Path>
    {
        let mut options = OpenOptions::new();
        options.write(true);

        if overwrite {
            options.create(true);
        } else {
            options.create_new(true);
        }

        let mut file = options.open(path).await?;
        let bytes = self.as_bytes()?;

        file.write_all(&bytes).await?;
        file.flush().await?;

        Ok(())
    }
}
