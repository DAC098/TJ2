create table users (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    username varchar not null unique,
    password varchar not null,
    version bigint not null default 0,
    created timestamp with time zone not null,
    updated timestamp with time zone
);

create table user_invites (
    token varchar primary key not null,
    name varchar not null unique,
    issued_on timestamp with time zone not null,
    expires_on timestamp with time zone,
    status smallint not null default 0,
    users_id bigint references users (id)
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

create table remote_servers (
    id bigint primary key generated always as identity,
    addr varchar unique not null,
    port int not null,
    secure boolean not null
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
    issued_on timestamp with time zone not null,
    expires_on timestamp with time zone not null,
    authenticated boolean not null default false,
    verified boolean not null default false
);

create table authz_roles (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    name varchar not null unique,
    created timestamp with time zone not null,
    updated timestamp with time zone
);

create table authz_permissions (
    id bigint primary key generated always as identity,
    role_id bigint not null references authz_roles (id),
    scope varchar not null,
    ability varchar not null,
    ref_id bigint,
    added timestamp with time zone,
    unique (role_id, scope, ability, ref_id)
);

create table user_roles (
    users_id bigint not null references users (id),
    role_id bigint not null references authz_roles (id),
    added timestamp with time zone not null,
    primary key (users_id, role_id)
);

create table group_roles (
    groups_id bigint not null references groups (id),
    role_id bigint not null references authz_roles (id),
    added timestamp with time zone not null,
    primary key (groups_id, role_id)
);

create table journals (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    users_id bigint not null references users (id),
    kind smallint not null default 0,
    name varchar not null,
    description varchar,
    created timestamp with time zone not null,
    updated timestamp with time zone,
    unique (users_id, name)
);

create table custom_fields (
    id bigint primary key generated always as identity,
    uid varchar not null unique,
    journals_id bigint not null references journals (id),
    name varchar not null,
    "order" integer default 0,
    config jsonb not null,
    description varchar,
    created timestamp with time zone not null,
    updated timestamp with time zone,
    unique (journals_id, name)
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
    status smallint not null default 0,
    name varchar,
    mime_type varchar not null,
    mime_subtype varchar not null,
    mime_param varchar,
    size bigint default 0,
    created timestamp with time zone not null,
    updated timestamp with time zone
);

create table custom_field_entries (
    custom_fields_id bigint not null references custom_fields (id),
    entries_id bigint not null references entries (id),
    value jsonb not null,
    created timestamp with time zone not null,
    updated timestamp with time zone,
    primary key (custom_fields_id, entries_id)
);

create table synced_journals (
    journals_id bigint not null references journals (id),
    server_id bigint not null references remote_servers (id),
    updated timestamp with time zone,
    primary key (journals_id, server_id)
);

create table synced_entries (
    entires_id bigint not null references entries (id),
    server_id bigint not null references remote_servers (id),
    status smallint not null,
    updated timestamp with time zone,
    primary key (entries_id, server_id)
);
