use std::fmt;

use bytes::BytesMut;
use postgres_types as pg_types;
use serde::{Serialize, Deserialize};

use crate::error::BoxDynError;
use crate::sec::sized_rand_bytes;

#[derive(Debug, thiserror::Error)]
#[error("invalid base64 string provided")]
pub struct InvalidBase64;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
#[serde(try_from = "&str", into = "String")]
pub struct Token<const N: usize>([u8; N]);

impl<const N: usize> Token<N> {
    pub fn new() -> Result<Self, rand::Error> {
        Ok(Self(sized_rand_bytes::<N>()?))
    }

    pub fn from_base64(given: &str) -> Result<Self, InvalidBase64> {
        let decoded = tj2_lib::string::from_base64_nopad(given)
            .ok_or(InvalidBase64)?;

        let bytes = decoded.try_into()
            .map_err(|_| InvalidBase64)?;

        Ok(Self(bytes))
    }

    pub fn as_base64(&self) -> String {
        tj2_lib::string::to_base64_nopad(&self.0)
    }
}

impl<const N: usize> TryFrom<&str> for Token<N> {
    type Error = InvalidBase64;

    fn try_from(given: &str) -> Result<Self, Self::Error> {
        Self::from_base64(given)
    }
}

impl<const N: usize> From<Token<N>> for String {
    fn from(token: Token<N>) -> Self {
        token.as_base64()
    }
}

impl<const N: usize> fmt::Display for Token<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

impl<const N: usize> pg_types::ToSql for Token<N> {
    fn to_sql(&self, ty: &pg_types::Type, w: &mut BytesMut) -> Result<pg_types::IsNull, BoxDynError> {
        self.0.as_slice()
            .to_sql(ty, w)
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&[u8] as pg_types::ToSql>::accepts(ty)
    }

    pg_types::to_sql_checked!();
}

impl<'a, const N: usize> pg_types::FromSql<'a> for Token<N> {
    fn from_sql(ty: &pg_types::Type, raw: &'a [u8]) -> Result<Self, BoxDynError> {
        let v = <Vec<u8> as pg_types::FromSql>::from_sql(ty, raw)?;

        let Ok(bytes) = v.try_into() else {
            return Err("invalid sql value for Token. expected bytea with 48 bytes".into());
        };

        Ok(Token(bytes))
    }

    fn accepts(ty: &pg_types::Type) -> bool {
        <&[u8] as pg_types::FromSql>::accepts(ty)
    }
}


