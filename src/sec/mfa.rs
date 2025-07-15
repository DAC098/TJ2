use std::fmt::Write;

use chrono::{DateTime, Utc};
use futures::StreamExt;
use rand::distributions::{Alphanumeric, DistString};

use crate::db;
use crate::db::ids::UserId;
use crate::sec::password;

pub mod otp;

pub const RECOVERY_CODE_AMOUNT: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error(transparent)]
    Hash(#[from] password::HashError),

    #[error(transparent)]
    Db(#[from] db::PgError),
}

pub async fn create_recovery(
    conn: &impl db::GenericClient,
    users_id: &UserId,
) -> Result<Vec<String>, RecoveryError> {
    let (codes, hashes) = gen_codes()?;

    let mut query = String::from("insert into authn_recovery (users_id, hash) values");
    let mut params: db::ParamsVec<'_> = vec![users_id];

    for (index, hash) in hashes.iter().enumerate() {
        if index != 0 {
            query.push_str(", ");
        }

        write!(&mut query, "($1, ${})", db::push_param(&mut params, hash)).unwrap();
    }

    conn.execute(&query, params.as_slice()).await?;

    Ok(codes)
}

pub fn gen_codes() -> Result<(Vec<String>, Vec<String>), password::HashError> {
    let mut codes = Vec::with_capacity(RECOVERY_CODE_AMOUNT);
    let mut hashes = Vec::with_capacity(RECOVERY_CODE_AMOUNT);
    let mut rng = rand::thread_rng();

    for _ in 0..RECOVERY_CODE_AMOUNT {
        let code = Alphanumeric.sample_string(&mut rng, 10);
        let hash = password::create(&code)?;

        codes.push(code);
        hashes.push(hash);
    }

    Ok((codes, hashes))
}

pub async fn delete_recovery(
    conn: &impl db::GenericClient,
    users_id: &UserId,
) -> Result<u64, db::PgError> {
    Ok(conn
        .execute(
            "delete from authn_recovery where users_id = $1",
            &[&users_id],
        )
        .await?)
}

pub struct Recovery {
    #[allow(dead_code)]
    users_id: UserId,
    hash: String,
    used_on: Option<DateTime<Utc>>,
}

impl Recovery {
    pub async fn retrieve_many(
        conn: &impl db::GenericClient,
        given: &UserId,
    ) -> Result<Vec<Self>, db::PgError> {
        let params: db::ParamsArray<'_, 1> = [given];
        let stream = conn
            .query_raw(
                "\
            select users_id, \
                   hash, \
                   used_on \
            from authn_recovery \
            where users_id = $1",
                params,
            )
            .await?;

        futures::pin_mut!(stream);

        let mut rtn = Vec::with_capacity(RECOVERY_CODE_AMOUNT);

        while let Some(maybe) = stream.next().await {
            let record = maybe?;

            rtn.push(Self {
                users_id: record.get(0),
                hash: record.get(1),
                used_on: record.get(2),
            });
        }

        Ok(rtn)
    }

    pub async fn mark_used(&mut self, conn: &impl db::GenericClient) -> Result<bool, db::PgError> {
        if self.used_on.is_some() {
            return Ok(false);
        }

        let used_on = Utc::now();

        conn.execute(
            "\
            update authn_recovery \
            set used_on = $2 \
            where hash = $1",
            &[&self.hash, &used_on],
        )
        .await?;

        self.used_on = Some(used_on);

        Ok(true)
    }
}

pub async fn verify_and_mark(
    conn: &impl db::GenericClient,
    users_id: &UserId,
    code: &str,
) -> Result<bool, RecoveryError> {
    for mut recovery_code in Recovery::retrieve_many(conn, users_id).await? {
        if recovery_code.used_on.is_some() {
            continue;
        }

        if password::verify(&recovery_code.hash, code)? {
            recovery_code.mark_used(conn).await?;

            return Ok(true);
        }
    }

    Ok(false)
}
