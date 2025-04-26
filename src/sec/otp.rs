use bytes::BytesMut;
use postgres_types as pg_types;
use rand::RngCore;
use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};

use crate::db;
use crate::db::ids::UserId;
use crate::error::BoxDynError;

pub use rust_otp::UnixTimestampError;

// we are using 25 bytes to not have padding in the base32 string sent to the
// user
pub const SECRET_LEN: usize = 25;

#[derive(Debug, Clone)]
pub struct Totp {
    pub users_id: UserId,
    pub algo: Algo,
    pub step: Step,
    pub digits: u8,
    pub secret: Secret,
}

#[derive(Debug, Clone, Copy, Serialize_repr, Deserialize_repr)]
#[repr(u64)]
pub enum Step {
    Small = 15,
    Medium = 30,
    Large = 45,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid step number")]
pub struct InvalidStep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Algo {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid algo number")]
pub struct InvalidAlgo;

#[derive(Debug, Clone)]
pub struct Secret([u8; SECRET_LEN]);

#[derive(Debug, thiserror::Error)]
pub enum TotpError {
    #[error("totp record already exists for this user")]
    AlreadyExists,

    #[error(transparent)]
    Db(#[from] db::PgError),
}

impl Totp {
    fn try_get_digits(given: i16) -> Option<u8> {
        if given < 6 || given > 8 {
            None
        } else {
            Some(given as u8)
        }
    }

    pub fn generate(users_id: UserId) -> Result<Self, rand::Error> {
        let digits = 6;
        let algo = Algo::default();
        let step = Step::default();
        let secret = Secret::gen()?;

        Ok(Self {
            users_id,
            algo,
            step,
            digits,
            secret,
        })
    }

    pub async fn retrieve(conn: &impl db::GenericClient, users_id: &UserId) -> Result<Option<Self>, db::PgError> {
        Ok(conn.query_opt(
            "\
            select authn_totp.users_id, \
                   authn_totp.algo, \
                   authn_totp.step, \
                   authn_totp.digits, \
                   authn_totp.secret \
            from authn_totp \
            where authn_totp.users_id = $1",
            &[users_id]
        ).await?.map(|record| {
            let digits = Self::try_get_digits(record.get(3)).expect(
                "invalid number of totp digits from database"
            );

            Self {
                users_id: record.get(0),
                algo: record.get(1),
                step: record.get(2),
                digits,
                secret: record.get(4),
            }
        }))
    }

    pub async fn exists(conn: &impl db::GenericClient, users_id: &UserId) -> Result<bool, db::PgError> {
        let result = conn.execute(
            "select authn_totp.users_id from authn_totp where users_id = $1",
            &[users_id]
        ).await?;

        Ok(result == 1)
    }

    pub async fn save(&self, conn: &impl db::GenericClient) -> Result<(), TotpError> {
        let result = conn.execute(
            "\
            insert into authn_totp (users_id, algo, step, digits, secret) values \
            ($1, $2, $3, $4, $5)",
            &[&self.users_id, &self.algo, &self.step, &db::U8toI16(&self.digits), &self.secret]
        ).await;

        match result {
            Ok(_count) => Ok(()),
            Err(err) => if let Some(kind) = db::ErrorKind::check(&err) {
                match kind {
                    db::ErrorKind::Unique(constraint) => match constraint {
                        "authn_totp_pkey" => Err(TotpError::AlreadyExists),
                        _ => Err(TotpError::Db(err)),
                    },
                    _ => Err(TotpError::Db(err)),
                }
            } else {
                Err(TotpError::Db(err))
            }
        }
    }

    pub async fn delete(&self, conn: &impl db::GenericClient) -> Result<(), db::PgError> {
        conn.execute(
            "delete from authn_totp where users_id = $1",
            &[&self.users_id]
        ).await?;

        Ok(())
    }

    pub fn verify<T>(&self, given: T) -> Result<bool, UnixTimestampError>
    where
        T: AsRef<str>
    {
        let settings = rust_otp::TotpSettings {
            algo: (&self.algo).into(),
            digits: self.digits as u32,
            step: (&self.step).into(),
            ..rust_otp::TotpSettings::default()
        };

        tracing::debug!("verifying code: {} {settings:#?}", given.as_ref());

        let result = rust_otp::verify_totp_code(&self.secret, given, &settings)?;

        tracing::debug!("verify result: {result:#?}");

        match result {
            rust_otp::VerifyResult::Valid => Ok(true),
            _ => Ok(false),
        }
    }
}

impl Default for Step {
    fn default() -> Self {
        Self::Medium
    }
}

impl From<&Step> for i32 {
    fn from(step: &Step) -> i32 {
        (*step as u64) as i32
    }
}

impl From<&Step> for u64 {
    fn from(step: &Step) -> u64 {
        *step as u64
    }
}

impl TryFrom<i32> for Step {
    type Error = InvalidStep;

    fn try_from(given: i32) -> Result<Self, Self::Error> {
        match given {
            15 => Ok(Self::Small),
            30 => Ok(Self::Medium),
            45 => Ok(Self::Large),
            _ => Err(InvalidStep)
        }
    }
}

impl pg_types::ToSql for Step {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        let num: i32 = self.into();

        num.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i32 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Step {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let value = <i32 as pg_types::FromSql>::from_sql(ty, raw)?;

        Ok(value.try_into()?)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i32 as pg_types::ToSql>::accepts(ty)
    }
}

impl Default for Algo {
    fn default() -> Self {
        Self::Sha1
    }
}

impl From<&Algo> for i32 {
    fn from(algo: &Algo) -> i32 {
        match algo {
            Algo::Sha1 => 0,
            Algo::Sha256 => 1,
            Algo::Sha512 => 2,
        }
    }
}

impl TryFrom<i32> for Algo {
    type Error = InvalidAlgo;

    fn try_from(given: i32) -> Result<Self, Self::Error> {
        match given {
            0 => Ok(Self::Sha1),
            1 => Ok(Self::Sha256),
            2 => Ok(Self::Sha512),
            _ => Err(InvalidAlgo)
        }
    }
}

impl From<&Algo> for rust_otp::Algo {
    fn from(algo: &Algo) -> rust_otp::Algo {
        match algo {
            Algo::Sha1 => rust_otp::Algo::SHA1,
            Algo::Sha256 => rust_otp::Algo::SHA256,
            Algo::Sha512 => rust_otp::Algo::SHA512,
        }
    }
}

impl pg_types::ToSql for Algo {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        let num: i32 = self.into();

        num.to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i32 as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Algo {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let value = <i32 as pg_types::FromSql>::from_sql(ty, raw)?;

        Ok(value.try_into()?)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <i32 as pg_types::ToSql>::accepts(ty)
    }
}

impl Secret {
    pub fn gen() -> Result<Self, rand::Error> {
        let mut bytes = [0; SECRET_LEN];

        rand::thread_rng().try_fill_bytes(&mut bytes)?;

        Ok(Self(bytes))
    }

    pub fn as_base32(&self) -> String {
        data_encoding::BASE32.encode(&self.0)
    }
}

impl AsRef<[u8]> for Secret {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl pg_types::ToSql for Secret {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        self.0.as_slice()
            .to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&[u8] as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Secret {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <&[u8] as pg_types::FromSql>::from_sql(ty, raw)?;

        let Ok(bytes) = v.try_into() else {
            return Err("invalid sql value for Secret. expected bytea with 24 bytes".into());
        };

        Ok(Self(bytes))
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&[u8] as pg_types::FromSql>::accepts(ty)
    }
}
