use crypto_box::{ChaChaBox, Nonce};
use crypto_box::aead::{Aead, AeadCore, OsRng};
use serde::{Serialize, Deserialize};

use crate::sec::sized_rand_bytes;

pub const DATA_LEN: usize = 32;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
#[serde(into = "Vec<u8>", try_from = "Vec<u8>")]
pub struct Data([u8; DATA_LEN]);

#[derive(Debug, thiserror::Error)]
#[error("invalid data length")]
pub struct InvalidDataLen;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct Challenge(Vec<u8>);

#[derive(Debug, thiserror::Error)]
pub enum ChallengeError {
    #[error("invalid data")]
    InvalidData,

    #[error(transparent)]
    Decrypt(#[from] DecryptError),
}

impl Data {
    pub fn new() -> Result<Self, rand::Error> {
        Ok(Self(sized_rand_bytes()?))
    }

    pub fn into_challenge(&self, user_box: &ChaChaBox) -> Result<Challenge, EncryptError> {
        Ok(Challenge(encrypt(user_box, &self.0)?))
    }
}

impl From<Data> for Vec<u8> {
    fn from(given: Data) -> Self {
        given.0.into()
    }
}

impl TryFrom<Vec<u8>> for Data {
    type Error = InvalidDataLen;

    fn try_from(given: Vec<u8>) -> Result<Self, Self::Error> {
        let bytes = given.try_into().map_err(|_| InvalidDataLen)?;

        Ok(Self(bytes))
    }
}

impl Challenge {
    pub fn into_data(self, user_box: &ChaChaBox) -> Result<Data, ChallengeError> {
        let bytes = decrypt(user_box, self.0)?;

        Ok(Data(bytes.try_into().map_err(|_| ChallengeError::InvalidData)?))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecryptError {
    #[error("invalid nonce")]
    InvalidNonce,

    #[error("failed to decrypt data")]
    Failed,
}

pub fn decrypt(user_box: &ChaChaBox, cipher: Vec<u8>) -> Result<Vec<u8>, DecryptError> {
    let Some((nonce, encrypted)) = cipher.split_at_checked(24) else {
        return Err(DecryptError::InvalidNonce);
    };

    let nonce = Nonce::from(TryInto::<[u8; 24]>::try_into(nonce).unwrap());

    user_box.decrypt(&nonce, encrypted)
        .map_err(|_| DecryptError::Failed)
}

#[derive(Debug, thiserror::Error)]
pub enum EncryptError {
    #[error("failed to encrypt data")]
    Failed
}

pub fn encrypt<T>(user_box: &ChaChaBox, data: T) -> Result<Vec<u8>, EncryptError>
where
    T: AsRef<[u8]>
{
    let nonce = ChaChaBox::generate_nonce(&mut OsRng);

    let encrypted = user_box.encrypt(&nonce, data.as_ref())
        .map_err(|_| EncryptError::Failed)?;

    let mut rtn = Vec::with_capacity(24 + encrypted.len());
    rtn.extend(nonce.as_slice());
    rtn.extend(encrypted);

    Ok(rtn)
}
