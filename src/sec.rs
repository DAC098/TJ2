use rand::RngCore;

pub mod authn;
pub mod authz;
pub mod otp;
pub mod password;
pub mod pki;

pub mod hash;
pub use hash::Hash;

pub fn fill_rand_bytes<T>(mut given: T) -> Result<(), rand::Error>
where
    T: AsMut<[u8]>,
{
    rand::thread_rng().try_fill_bytes(given.as_mut())
}

pub fn sized_rand_bytes<const N: usize>() -> Result<[u8; N], rand::Error> {
    let mut data = [0; N];

    fill_rand_bytes(&mut data)?;

    Ok(data)
}
