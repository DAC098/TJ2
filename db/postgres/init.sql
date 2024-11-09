create table users (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    username varchar not null unique,
    password varchar not null,
    version bigint not null default 0
);

create table groups (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    name varchar not null unique,
    created timestamp with time zone not null,
    updated timestamp with time zone
);

create table group_users (
    users_id bigint not null references users (id),
    groups_id bigint not null references groups (id),
    added timestamp with time zone not null,
    primary key (users_id, groups_id)
);

create table authn_totp (
    users_id bigint primary key not null references users (id),
    algo int not null,
    step int not null,
    digits int not null,
    secret bytea not null
);

create table authn_sessions (
    token bytea primary key not null,
    users_id bigint not null references users (id),
    dropped boolean not null default false,
    issued_on timestamp with time zone not null,
    expires_on timestamp with time zone not null,
    authenticated boolean not null default false,
    verified boolean not null default false
);

create table authz_roles (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    name varchar not null unique
);

create table authz_permissions (
    id bigint primary key generated always as identity,
    role_id bigint not null references authz_roles (id),
    scope varchar not null,
    ability varchar not null,
    ref_id bigint,
    unique (role_id, scope, ability, ref_id)
);

create table user_roles (
    users_id bigint not null references users (id),
    role_id bigint not null references authz_roles (id),
    primary key (users_id, role_id)
);

create table group_roles (
    groups_id bigint not null references groups (id),
    role_id bigint not null references authz_roles (id),
    primary key (groups_id, role_id)
);

create table journals (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    users_id bigint not null references users (id),
    name varchar not null,
    created timestamp with time zone not null,
    updated timestamp with time zone,
    unique (users_id, name)
);

create table entries (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    journals_id bigint not null references journals (id),
    users_id bigint not null references users (id),
    entry_date date not null,
    title varchar,
    contents varchar,
    created timestamp with time zone not null,
    updated timestamp with time zone,
    unique (journals_id, entry_date)
);

create table entry_tags (
    entries_id bigint not null references entries (id),
    key varchar not null,
    value varchar,
    created timestamp with time zone not null,
    updated timestamp with time zone,
    primary key (entries_id, key)
);

create table file_entries (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    entries_id bigint not null references entries (id),
    name varchar,
    mime_type varchar not null,
    mime_subtype varchar not null,
    mime_param varchar,
    size bigint default 0,
    created timestamp with time zone not null,
    updated timestamp with time zone
);
