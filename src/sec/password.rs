use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, SaltString};
use rand::rngs::OsRng;

#[derive(Debug, thiserror::Error)]
#[error("an error occurred when attempt to create the argon2 hash")]
pub struct HashError;

pub fn create<P>(password: P) -> Result<String, HashError>
where
    P: AsRef<[u8]>
{
    let salt = SaltString::generate(&mut OsRng);
    let config = get_config();

    match config.hash_password(password.as_ref(), &salt) {
        Ok(hash) => Ok(hash.to_string()),
        Err(_err) => Err(HashError)
    }
}

fn get_config() -> Argon2<'static> {
    Argon2::default()
}
