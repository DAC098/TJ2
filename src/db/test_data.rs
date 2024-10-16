use chrono::{Days, DateTime, NaiveTime, NaiveDate, Utc};
use rand::Rng;
use rand::rngs::ThreadRng;
use rand::distributions::{Alphanumeric, Bernoulli};
use sqlx::Row;

use super::DbConn;
use super::ids;

use crate::error::{Error, Context};
use crate::journal::Journal;
use crate::user::{User, Group, assign_user_group};
use crate::sec::password;
use crate::sec::authz::{Role, Scope, Ability, create_permissions, assign_group_role};

pub async fn create(conn: &mut DbConn, rng: &mut ThreadRng) -> Result<(), Error> {
    let password = "password";

    let journalists_group = Group::create(conn, "journalists")
        .await
        .context("failed to create journalists group")?;
    let journalists_role = Role::create(conn, "journalists")
        .await
        .context("failed to create journalists role")?;

    assign_group_role(conn, journalists_group.id, journalists_role.id)
        .await
        .context("failed to assign journalists group to journalists role")?;

    let permissions = vec![
        (Scope::Journals, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ]),
        (Scope::Entries, vec![
            Ability::Create,
            Ability::Read,
            Ability::Update,
            Ability::Delete,
        ])
    ];

    create_permissions(conn, journalists_role.id, permissions)
        .await
        .context("failed to create permissions for journalists role")?;

    for _ in 0..10 {
        let username = gen_username(rng);
        let user = create_user(conn, &username, password).await?;

        tracing::debug!("create new user: {}", user.id);

        assign_user_group(conn, user.id, journalists_group.id)
            .await
            .context("failed to assign test user to journalists group")?;

        create_journal(conn, rng, user.id).await?;
    }

    Ok(())
}

pub async fn create_journal(
    conn: &mut DbConn,
    rng: &mut ThreadRng,
    users_id: ids::UserId,
) -> Result<(), Error> {
    let journal = Journal::create(conn, users_id, "default")
        .await
        .context("failed to create journal for test user")?;

    let today = Utc::now();
    let total_entries = rng.gen_range(50..=240) + 1;

    for count in 1..total_entries {
        let date = today.date_naive()
            .checked_sub_days(Days::new(count))
            .unwrap();

        tracing::debug!("creating entry: {date}");

        create_journal_entry(conn, rng, journal.id, users_id, date).await?;
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
) -> Result<User, Error> {
    let hash = password::create(password)
        .context("failed to create argon2 hash")?;

    User::create(conn, username, &hash, 0)
        .await
        .context("failed to create user")
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
