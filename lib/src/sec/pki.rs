use std::path::Path;

use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};
use rand::RngCore;

use crate::string::{to_hex_str, from_hex_str};

pub type PrivateKey = crypto_box::SecretKey;

pub fn gen_private_key() -> Result<PrivateKey, rand::Error> {
    let mut bytes = [0u8; crypto_box::KEY_SIZE];

    rand::rngs::OsRng.try_fill_bytes(&mut bytes)?;

    Ok(PrivateKey::from_bytes(bytes))
}

#[derive(Debug, thiserror::Error)]
pub enum LoadKeyError {
    #[error("non-hexidecimal data in private key")]
    InvalidContents,

    #[error("invalid number of bytes from private key")]
    InvalidLength,

    #[error(transparent)]
    Io(#[from] std::io::Error)
}

pub async fn load_private_key<P>(path: P) -> Result<PrivateKey, LoadKeyError>
where
    P: AsRef<Path>
{
    let contents = tokio::fs::read_to_string(path).await?;

    let Some(bytes) = from_hex_str(&contents) else {
        return Err(LoadKeyError::InvalidContents);
    };

    PrivateKey::from_slice(bytes.as_slice())
        .map_err(|_| LoadKeyError::InvalidLength)
}

pub async fn save_private_key<P>(path: P, key: &PrivateKey, overwrite: bool) -> Result<(), std::io::Error>
where
    P: AsRef<Path>
{
    let contents = to_hex_str(key.to_bytes());
    let mut options = OpenOptions::new();
    options.write(true);

    if overwrite {
        options.create(true);
    } else {
        options.create_new(true);
    }

    let file = options.open(path).await?;
    let mut writer = BufWriter::new(file);

    writer.write_all(contents.as_bytes()).await?;
    writer.flush().await?;

    Ok(())
}
