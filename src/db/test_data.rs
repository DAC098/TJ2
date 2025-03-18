use chrono::{Days, DateTime, NaiveTime, NaiveDate, Utc};
use rand::Rng;
use rand::rngs::ThreadRng;
use rand::distributions::{Alphanumeric, Bernoulli};

use super::{GenericClient, ids};

use crate::error::{Error, Context};
use crate::journal::{custom_field, CustomField, LocalJournal};
use crate::user::{User, Group, assign_user_group};
use crate::sec::password;
use crate::sec::authz::{Role, Scope, Ability};
use crate::state;

pub async fn create(
    state: &state::SharedState,
    conn: &impl GenericClient,
    rng: &mut ThreadRng
) -> Result<(), Error> {
    let password = "password";

    let journalists_group = Group::create(conn, "journalists")
        .await
        .context("failed to create journalists group")?
        .context("journalists group already exists")?;
    let journalists_role = Role::create(conn, "journalists")
        .await
        .context("failed to create journalists role")?
        .context("journalists role already exists")?;

    journalists_role.assign_group(conn, journalists_group.id)
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

    journalists_role.assign_permissions(conn, &permissions)
        .await
        .context("failed to create permissions for journalists role")?;

    for _ in 0..10 {
        let username = gen_username(rng);
        let user = create_user(conn, &username, password).await?;

        tracing::debug!("create new user: {}", user.id);

        assign_user_group(conn, user.id, journalists_group.id)
            .await
            .context("failed to assign test user to journalists group")?;

        create_journal(state, conn, rng, user.id).await?;
    }

    Ok(())
}

pub async fn create_journal(
    state: &state::SharedState,
    conn: &impl GenericClient,
    rng: &mut ThreadRng,
    users_id: ids::UserId
) -> Result<(), Error> {
    let options = LocalJournal::create_options(users_id, "default")
        .description("the default journal");
    let journal = LocalJournal::create(conn, options)
        .await
        .context("failed to create journal for test user")?;

    let custom_fields = vec![
        CustomField::create(conn, CustomField::create_options(
            journal.id,
            "mood",
            (custom_field::IntegerType {
                minimum: Some(1),
                maximum: Some(10),
            }).into()
        ))
            .await
            .context("failed to create mood field for journal")?,
        CustomField::create(conn, CustomField::create_options(
            journal.id,
            "sleep",
            (custom_field::TimeRangeType {
                show_diff: true,
            }).into()
        ))
            .await
            .context("failed to create sleep field for journal")?,
    ];

    let journal_dir = state.storage()
        .journal_dir(journal.id);

    journal_dir.create()
        .await
        .context("failed to create journal directory")?;

    let today = Utc::now();
    let total_entries = rng.gen_range(50..=730) + 1;

    for count in 1..total_entries {
        let date = today.date_naive()
            .checked_sub_days(Days::new(count))
            .unwrap();

        //tracing::debug!("creating entry: {date}");

        create_journal_entry(conn, rng, journal.id, users_id, date, &custom_fields).await?;
    }

    tracing::info!("created {total_entries} entries");

    Ok(())
}

async fn create_journal_entry(
    conn: &impl GenericClient,
    rng: &mut ThreadRng,
    journals_id: ids::JournalId,
    users_id: ids::UserId,
    date: NaiveDate,
    custom_fields: &Vec<CustomField>,
) -> Result<(), Error> {
    let dist = Bernoulli::from_ratio(6, 10)
        .context("failed to create Bernoulli distribution")?;

    let uid = ids::EntryUid::gen();
    let created = gen_created(rng, date);
    let updated = gen_updated(rng, dist, date);
    let title = gen_entry_title(rng, dist);

    let result = conn.query_one(
        "\
        insert into entries (uid, journals_id, users_id, title, entry_date, created, updated) \
        values ($1, $2, $3, $4, $5, $6, $7) \
        returning id",
        &[
            &uid,
            &journals_id,
            &users_id,
            &title,
            &date,
            &created,
            &updated
        ]
    )
        .await
        .context("failed to insert new entry into journal")?;

    let entries_id: ids::EntryId = result.get(0);

    for _ in 0..rng.gen_range(0..5) {
        let created = Utc::now();
        let key = gen_tag_key(rng);
        let value = gen_tag_value(rng, dist);

        conn.execute(
            "\
            insert into entry_tags (entries_id, key, value, created) \
            values ($1, $2, $3, $4)",
            &[&entries_id, &key, &value, &created]
        )
            .await
            .context("failed to insert journal tag")?;
    }

    for field in custom_fields {
        let created = Utc::now();
        let value = gen_custom_field_value(
            rng,
            &field.config,
            date
        );

        conn.execute(
            "\
            insert into custom_field_entries (custom_fields_id, entries_id, value, created) \
            values ($1, $2, $3, $4)",
            &[&field.id, &entries_id, &value, &created]
        )
            .await
            .context("failed to insert custom field value")?;
    }

    Ok(())
}

async fn create_user(
    conn: &impl GenericClient,
    username: &str,
    password: &str,
) -> Result<User, Error> {
    let hash = password::create(password)
        .context("failed to create argon2 hash")?;

    User::create(conn, username, &hash, 0)
        .await
        .context("failed to create user")?
        .context("user already exists?")
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

fn gen_custom_field_value(
    rng: &mut ThreadRng,
    config: &custom_field::Type,
    date: NaiveDate,
) -> custom_field::Value {
    match config {
        custom_field::Type::Integer(ty) => match (&ty.minimum, &ty.maximum) {
            (Some(min), Some(max)) => {
                let value = rng.gen_range(*min..*max);

                (custom_field::IntegerValue { value }).into()
            }
            (Some(min), None) => {
                let upper = rng.gen_range(5..10);
                let value = rng.gen_range(*min..(*min + upper));

                (custom_field::IntegerValue { value }).into()
            }
            (None, Some(max)) => {
                let lower = rng.gen_range(5..10);
                let value = rng.gen_range((*max - lower)..*max);

                (custom_field::IntegerValue { value }).into()
            }
            (None, None) => {
                let value = rng.gen_range(-10..10);

                (custom_field::IntegerValue { value }).into()
            }
        }
        custom_field::Type::IntegerRange(ty) => match (&ty.minimum, &ty.maximum) {
            (Some(min), Some(max)) => {
                let diff = *max - *min;

                if diff < 2 {
                    (custom_field::IntegerRangeValue {
                        low: *min,
                        high: *max,
                    }).into()
                } else {
                    let mid = diff / 2;
                    let low = rng.gen_range(*min..mid);
                    let high = rng.gen_range(mid..*max);

                    (custom_field::IntegerRangeValue { low, high }).into()
                }
            }
            (Some(min), None) => {
                let diff = rng.gen_range(2..8);
                let upper = rng.gen_range(2..8);

                let mid = *min + diff;

                let low = rng.gen_range(*min..mid);
                let high = rng.gen_range(mid..(mid + upper));

                (custom_field::IntegerRangeValue { low, high }).into()
            }
            (None, Some(max)) => {
                let diff = rng.gen_range(2..8);
                let lower = rng.gen_range(2..8);

                let mid = *max - diff;

                let low = rng.gen_range((mid - lower)..mid);
                let high = rng.gen_range(mid..*max);

                (custom_field::IntegerRangeValue { low, high }).into()
            }
            (None, None) => {
                let low = rng.gen_range(1..5);
                let high = rng.gen_range(5..10);

                (custom_field::IntegerRangeValue { low, high }).into()
            }
        }
        custom_field::Type::Float(ty) => match (&ty.minimum, &ty.maximum) {
            (Some(min), Some(max)) => {
                let value = rng.gen_range(*min..*max);

                (custom_field::FloatValue { value }).into()
            }
            (Some(min), None) => {
                let upper = rng.gen_range(5.0..10.0);
                let value = rng.gen_range(*min..(*min + upper));

                (custom_field::FloatValue { value }).into()
            }
            (None, Some(max)) => {
                let lower = rng.gen_range(5.0..10.0);
                let value = rng.gen_range((*max - lower)..*max);

                (custom_field::FloatValue { value }).into()
            }
            (None, None) => {
                let value = rng.gen_range(1.0..10.0);

                (custom_field::FloatValue { value }).into()
            }
        }
        custom_field::Type::FloatRange(ty) => match (&ty.minimum, &ty.maximum) {
            (Some(min), Some(max)) => {
                let diff = *max - *min;

                if diff < 2.0 {
                    (custom_field::FloatRangeValue {
                        low: *min,
                        high: *max
                    }).into()
                } else {
                    let mid = diff / 2.0;
                    let low = rng.gen_range(*min..mid);
                    let high = rng.gen_range(mid..*max);

                    (custom_field::FloatRangeValue { low, high }).into()
                }
            }
            (Some(min), None) => {
                let diff = rng.gen_range(2.0..8.0);
                let upper = rng.gen_range(2.0..8.0);

                let mid = *min + diff;

                let low = rng.gen_range(*min..mid);
                let high = rng.gen_range(mid..(mid + upper));

                (custom_field::FloatRangeValue { low, high }).into()
            }
            (None, Some(max)) => {
                let diff = rng.gen_range(2.0..8.0);
                let lower = rng.gen_range(2.0..8.0);

                let mid = *max - diff;

                let low = rng.gen_range((mid - lower)..mid);
                let high = rng.gen_range(mid..*max);

                (custom_field::FloatRangeValue { low, high }).into()
            }
            (None, None) => {
                let low = rng.gen_range(1.0..5.0);
                let high = rng.gen_range(5.0..10.0);

                (custom_field::FloatRangeValue { low, high }).into()
            }
        }
        custom_field::Type::Time(_ty) => {
            let hours = rng.gen_range(1..=23);
            let minutes = rng.gen_range(1..60);
            let seconds = rng.gen_range(1..60);

            let value = date.and_hms_opt(hours, minutes, seconds)
                .unwrap()
                .and_utc();

            (custom_field::TimeValue { value }).into()
        }
        custom_field::Type::TimeRange(_ty) => {
            let hours = rng.gen_range(6..=8);
            let minuts = rng.gen_range(1..60);
            let seconds = rng.gen_range(1..60);

            let start_hr = rng.gen_range(7..10);
            let start_min = rng.gen_range(1..60);
            let start_sec = rng.gen_range(1..60);

            let low = date.and_hms_opt(start_hr, start_min, start_sec)
                .unwrap()
                .and_utc();
            let high = low + chrono::Duration::seconds(
                hours * 60 * 60 +
                minuts * 60 +
                seconds
            );

            (custom_field::TimeRangeValue { low, high }).into()
        }
    }
}
