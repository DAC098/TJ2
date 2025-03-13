use bytes::BytesMut;
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::error::BoxDynError;

#[derive(Debug, Eq, Serialize, Deserialize)]
pub struct Hash(pub blake3::Hash);

impl Hash {
    pub fn from_hex<T>(given: T) -> Result<Self, blake3::HexError>
    where
        T: AsRef<[u8]>
    {
        Ok(Self(blake3::Hash::from_hex(given)?))
    }
}

impl From<blake3::Hash> for Hash {
    fn from(given: blake3::Hash) -> Self {
        Self(given)
    }
}

impl PartialEq<blake3::Hash> for Hash {
    fn eq(&self, other: &blake3::Hash) -> bool {
        self.0.eq(other)
    }
}

impl PartialEq for Hash {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl pg_types::ToSql for Hash {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        self.0.to_hex()
            .as_str()
            .to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a> pg_types::FromSql<'a> for Hash {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <&str as pg_types::FromSql>::from_sql(ty, raw)?;

        Ok(Self(blake3::Hash::from_hex(v)?))
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&str as pg_types::FromSql>::accepts(ty)
    }
}
