use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::str::FromStr;

use postgres_types::{ToSql, FromSql};
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
            ToSql, FromSql,
            Serialize, Deserialize,
        )]
        #[sqlx(transparent)]
        #[postgres(transparent)]
        #[serde(try_from = "i64", into = "i64")]
        pub struct $name(i64);

        impl $name {
            pub fn new(value: i64) -> Result<Self, InvalidIdInteger> {
                if value < 0 {
                    Err(InvalidIdInteger)
                } else {
                    Ok(Self(value))
                }
            }

            pub fn zero() -> Self {
                Self(0)
            }

            pub fn is_zero(&self) -> bool {
                self.0 == 0
            }

            pub fn inner(&self) -> &i64 {
                &self.0
            }
        }

        impl AsRef<i64> for $name {
            fn as_ref(&self) -> &i64 {
                self.inner()
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
                    if int < 0 {
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
            ToSql, FromSql,
            Serialize, Deserialize,
        )]
        #[sqlx(transparent)]
        #[postgres(transparent)]
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
    ($name:ident, $id:ty, $uid:ty) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            id: $id,
            uid: $uid,
        }

        impl $name {
            pub fn new(id: $id, uid: $uid) -> Self {
                $name { id, uid }
            }

            pub fn id(&self) -> &$id {
                &self.id
            }

            pub fn uid(&self) -> &$uid {
                &self.uid
            }

            pub fn into_id(self) -> $id {
                self.id
            }

            pub fn into_uid(self) -> $uid {
                self.uid
            }
        }

        impl std::cmp::PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.id.eq(&other.id)
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
                self.id.cmp(&other.id)
            }
        }

        impl std::cmp::PartialEq<$id> for $name {
            fn eq(&self, other: &$id) -> bool {
                self.id.eq(other)
            }
        }

        impl std::cmp::PartialEq<$uid> for $name {
            fn eq(&self, other: &$uid) -> bool {
                self.uid.eq(other)
            }
        }

        impl From<$name> for $id {
            fn from(value: $name) -> $id {
                value.id
            }
        }

        impl From<$name> for $uid {
            fn from(value: $name) -> $uid {
                value.uid
            }
        }

        impl From<($id, $uid)> for $name {
            fn from((id, uid): ($id, $uid)) -> Self {
                $name { id, uid }
            }
        }

        impl std::hash::Hash for $name {
            fn hash<H>(&self, state: &mut H)
            where
                H: std::hash::Hasher
            {
                self.id.hash(state);
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                write!(f, "id: {} uid: {}", self.id, self.uid)
            }
        }
    }
}

id_type!(UserId);
uid_type!(UserUid);
set_type!(UserSet, UserId, UserUid);

id_type!(GroupId);
uid_type!(GroupUid);

id_type!(JournalId);
uid_type!(JournalUid);
set_type!(JournalSet, JournalId, JournalUid);

id_type!(EntryId);
uid_type!(EntryUid);
set_type!(EntrySet, EntryId, EntryUid);

id_type!(FileEntryId);
uid_type!(FileEntryUid);

id_type!(RoleId);
uid_type!(RoleUid);

id_type!(PermissionId);

id_type!(CustomFieldId);
uid_type!(CustomFieldUid);

/// creates a list of unique ids from a given list
///
/// if a current dictionary of known ids is provided then it will create a list
/// of known ids, unknown ids, and missing ids with current representing the
/// missing ids.
pub fn unique_ids<K, V>(
    ids: Vec<K>,
    current: Option<&mut HashMap<K, V>>,
) -> (HashSet<K>, Vec<K>, HashMap<K, V>)
where
    K: std::cmp::Eq + std::hash::Hash + std::marker::Copy
{
    let mut set = HashSet::with_capacity(ids.len());
    let mut list = Vec::with_capacity(ids.len());
    let mut common = HashMap::new();

    if let Some(current) = current {
        common.reserve(current.len());

        for id in ids {
            if let Some(record) = current.remove(&id) {
                common.insert(id, record);
            } else {
                if set.insert(id) {
                    list.push(id);
                }
            }
        }

        (set, list, common)
    } else {
        for id in ids {
            if set.insert(id) {
                list.push(id);
            }
        }

        (set, list, common)
    }
}
