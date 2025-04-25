use argon2::{Argon2, PasswordHash, PasswordVerifier};
use argon2::password_hash::{PasswordHasher, SaltString};
use rand::rngs::OsRng;

pub use argon2::password_hash::Error as HashError;

fn get_config() -> Argon2<'static> {
    Argon2::default()
}

pub fn create<P>(password: P) -> Result<String, HashError>
where
    P: AsRef<[u8]>
{
    let salt = SaltString::generate(&mut OsRng);
    let config = get_config();

    Ok(config.hash_password(password.as_ref(), &salt)?.to_string())
}

pub fn verify<P>(password: &str, verify: P) -> Result<bool, HashError>
where
    P: AsRef<[u8]>
{
    let config = get_config();
    let hash = PasswordHash::new(password)?;

    if let Err(err) = config.verify_password(verify.as_ref(), &hash) {
        match err {
            HashError::Password => Ok(false),
            _ => Err(err.into())
        }
    } else {
        Ok(true)
    }
}
