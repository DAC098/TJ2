use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, SaltString};
use chrono::{Days, DateTime, NaiveTime, NaiveDate, Utc};
use rand::Rng;
use rand::rngs::{OsRng, ThreadRng};
use rand::distributions::{Alphanumeric, Bernoulli};
use sqlx::Row;

use super::DbConn;
use super::ids;

use crate::error::{Error, Context};

pub async fn create_rand_users(
    conn: &mut DbConn,
    rng: &mut ThreadRng
) -> Result<(), Error> {
    let password = "password";

    for _ in 0..10 {
        let username = gen_username(rng);

        let users_id = create_user(conn, &username, password).await?;

        tracing::debug!("create new user: id {users_id}");

        let journals_id = create_journal(conn, users_id).await?;

        create_data(conn, rng, journals_id, users_id)
            .await
            .context("failed to create test data for rand user")?;
    }

    Ok(())
}

pub async fn create_journal(
    conn: &mut DbConn,
    users_id: ids::UserId,
) -> Result<ids::JournalId, Error> {
    let uid = ids::JournalUid::gen();
    let created = Utc::now();

    let result = sqlx::query(
        "\
        insert into journals (uid, users_id, name, created) values \
        (?1, ?2, ?3, ?4) \
        returning id"
    )
        .bind(uid)
        .bind(users_id)
        .bind("default")
        .bind(created)
        .fetch_one(conn)
        .await
        .context("failed to create default journal for user")?;

    Ok(result.get(0))
}

pub async fn create_data(
    conn: &mut DbConn,
    rng: &mut ThreadRng,
    journals_id: ids::JournalId,
    users_id: ids::UserId,
) -> Result<(), Error> {
    let today = Utc::now();
    let total_entries = rng.gen_range(50..=240) + 1;

    for count in 1..total_entries {
        let date = today.date_naive()
            .checked_sub_days(Days::new(count))
            .unwrap();

        tracing::debug!("creating entry: {date}");

        create_journal_entry(conn, rng, journals_id, users_id, date).await?;
    }

    tracing::info!("created {total_entries} entries");

    Ok(())
}

async fn create_journal_entry(
    conn: &mut DbConn,
    rng: &mut ThreadRng,
    journals_id: ids::JournalId,
    users_id: ids::UserId,
    date: NaiveDate
) -> Result<(), Error> {
    let dist = Bernoulli::from_ratio(6, 10)
        .context("failed to create Bernoulli distribution")?;

    let uid = ids::EntryUid::gen();
    let created = gen_created(rng, date);
    let updated = gen_updated(rng, dist, date);
    let title = gen_entry_title(rng, dist);

    let result = sqlx::query(
        "\
        insert into entries (uid, journals_id, users_id, title, entry_date, created, updated) \
        values (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
        returning id"
    )
        .bind(uid)
        .bind(journals_id)
        .bind(users_id)
        .bind(title)
        .bind(date)
        .bind(created)
        .bind(updated)
        .fetch_one(&mut *conn)
        .await
        .context("failed to insert new entry into journal")?;

    let entries_id: ids::EntryId = result.get(0);

    for _ in 0..rng.gen_range(0..5) {
        let created = Utc::now();
        let key = gen_tag_key(rng);
        let value = gen_tag_value(rng, dist);

        sqlx::query(
            "\
            insert into entry_tags (entries_id, key, value, created) \
            values (?1, ?2, ?3, ?4)"
        )
            .bind(entries_id)
            .bind(key)
            .bind(value)
            .bind(created)
            .execute(&mut *conn)
            .await
            .context("failed to insert journal tag")?;
    }

    Ok(())
}

async fn create_user(
    conn: &mut DbConn,
    username: &str,
    password: &str
) -> Result<ids::UserId, Error> {
    let uid = ids::UserUid::gen();
    let salt = SaltString::generate(&mut OsRng);
    let config = Argon2::default();
    let password_hash = match config.hash_password(password.as_bytes(), &salt) {
        Ok(hashed) => hashed.to_string(),
        Err(err) => {
            tracing::debug!("argon2 hash_password error: {err:#?}");

            return Err(Error::context("failed to hash user password"))?;
        }
    };

    let result = sqlx::query(
        "\
        insert into users (uid, username, password, version) \
        values (?1, ?2, ?3, ?4) \
        returning id"
    )
        .bind(uid)
        .bind(username)
        .bind(&password_hash)
        .bind(0)
        .fetch_one(&mut *conn)
        .await
        .context("failed to create user")?;

    Ok(result.get(0))
}

fn gen_username(rng: &mut ThreadRng) -> String {
    let len = rng.gen_range(8..16);

    (0..len).map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}

fn gen_tag_key(rng: &mut ThreadRng) -> String {
    let len = rng.gen_range(4..12);

    (0..len).map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}

fn gen_tag_value(rng: &mut ThreadRng, dist: Bernoulli) -> Option<String> {
    if rng.sample(dist) {
        let len = rng.gen_range(8..24);

        let v: String = (0..len)
            .map(|_| rng.sample(Alphanumeric) as char)
            .collect();

        Some(v)
    } else {
        None
    }
}

fn gen_naive_time(rng: &mut ThreadRng) -> NaiveTime {
    let hour = rng.gen_range(7..18);
    let minute = rng.gen_range(0..60);
    let second = rng.gen_range(0..60);
    let millis = rng.gen_range(0..1000);

    NaiveTime::from_hms_milli_opt(hour, minute, second, millis).unwrap()
}

fn gen_created(rng: &mut ThreadRng, date: NaiveDate) -> DateTime<Utc> {
    date.and_time(gen_naive_time(rng))
        .and_utc()
}

fn gen_updated(rng: &mut ThreadRng, dist: Bernoulli, date: NaiveDate) -> Option<DateTime<Utc>> {
    if rng.sample(dist) {
        let days = rng.gen_range(0..3);
        let time = gen_naive_time(rng);

        Some(date.checked_add_days(Days::new(days))
            .unwrap()
            .and_time(time)
            .and_utc())
    } else {
        None
    }
}

fn gen_entry_title(rng: &mut ThreadRng, dist: Bernoulli) -> Option<String> {
    if rng.sample(dist) {
        let len = rng.gen_range(12..24);

        Some((0..len).map(|_| rng.sample(Alphanumeric) as char)
            .collect())
    } else {
        None
    }
}
