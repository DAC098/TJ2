use std::fmt::{Display, Formatter, Result as FmtResult};
use std::str::FromStr;

use serde::{Serialize, Deserialize};

pub const UID_SIZE: usize = 16;
pub const UID_ALPHABET: [char; 63] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    '_'
];

#[derive(Debug, thiserror::Error)]
#[error("provided integer is less than or equal to zero")]
pub struct InvalidIdInteger;

#[derive(Debug, thiserror::Error)]
#[error("provided string contains invalid characters or is less than or equal to zero")]
pub struct InvalidIdString;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone, Copy,
            PartialEq, Eq, PartialOrd, Ord, Hash,
            sqlx::Type,
            Serialize, Deserialize,
        )]
        #[sqlx(transparent)]
        #[serde(try_from = "i64", into = "i64")]
        pub struct $name(i64);

        impl $name {
            pub fn new(value: i64) -> Result<Self, InvalidIdInteger> {
                if value <= 0 {
                    Err(InvalidIdInteger)
                } else {
                    Ok(Self(value))
                }
            }

            pub fn inner(&self) -> &i64 {
                &self.0
            }
        }

        impl TryFrom<i64> for $name {
            type Error = InvalidIdInteger;

            fn try_from(value: i64) -> Result<Self, Self::Error> {
                $name::new(value)
            }
        }

        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = InvalidIdString;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if let Ok(int) = FromStr::from_str(s) {
                    if int <= 0 {
                        Err(InvalidIdString)
                    } else {
                        Ok($name(int))
                    }
                } else {
                    Err(InvalidIdString)
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("provided string contains invalid characters or invalid length")]
pub struct InvalidUidString;

macro_rules! uid_type {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            PartialEq, Eq, PartialOrd, Ord, Hash,
            sqlx::Type,
            Serialize, Deserialize,
        )]
        #[sqlx(transparent)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            fn check(given: &str) -> bool {
                let mut count: usize = 0;

                for ch in given.chars() {
                    if !(ch == '_' || ch.is_ascii_alphanumeric()) {
                        return false;
                    }

                    count += 1;
                }

                if count != UID_SIZE {
                    return false;
                }

                true
            }

            pub fn gen() -> Self {
                Self(nanoid::format(nanoid::rngs::default, &UID_ALPHABET, UID_SIZE))
            }

            pub fn new(given: String) -> Result<Self, InvalidUidString> {
                if !Self::check(&given) {
                    Err(InvalidUidString)
                } else {
                    Ok(Self(given))
                }
            }

            pub fn inner(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidUidString;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                if !Self::check(&value) {
                    Err(InvalidUidString)
                } else {
                    Ok(Self(value))
                }
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = InvalidUidString;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                if !Self::check(s) {
                    Err(InvalidUidString)
                } else {
                    Ok(Self(s.to_owned()))
                }
            }
        }
    }
}

macro_rules! set_type {
    ($name:ident, $local:ty, $uid:ty) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            local: $local,
            uid: $uid,
        }

        impl $name {
            pub fn new(local: $local, uid: $uid) -> Self {
                $name { local, uid }
            }

            pub fn local(&self) -> &$local {
                &self.local
            }

            pub fn uid(&self) -> &$uid {
                &self.uid
            }

            pub fn into_local(self) -> $local {
                self.local
            }

            pub fn into_uid(self) -> $uid {
                self.uid
            }
        }

        impl std::cmp::PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.local.eq(&other.local)
            }
        }

        impl std::cmp::Eq for $name {}

        impl std::cmp::PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl std::cmp::Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.local.cmp(&other.local)
            }
        }

        impl std::cmp::PartialEq<$local> for $name {
            fn eq(&self, other: &$local) -> bool {
                self.local.eq(other)
            }
        }

        impl std::cmp::PartialEq<$uid> for $name {
            fn eq(&self, other: &$uid) -> bool {
                self.uid.eq(other)
            }
        }

        impl From<$name> for $local {
            fn from(value: $name) -> $local {
                value.local
            }
        }

        impl From<$name> for $uid {
            fn from(value: $name) -> $uid {
                value.uid
            }
        }

        impl From<($local, $uid)> for $name {
            fn from((local, uid): ($local, $uid)) -> Self {
                $name { local, uid }
            }
        }

        impl std::hash::Hash for $name {
            fn hash<H>(&self, state: &mut H)
            where
                H: std::hash::Hasher
            {
                self.local.hash(state);
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                write!(f, "local: {} uid: {}", self.local, self.uid)
            }
        }
    }
}

id_type!(UserId);
uid_type!(UserUid);
set_type!(UserSet, UserId, UserUid);
