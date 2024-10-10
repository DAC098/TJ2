CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    uid VARCHAR NOT NULL UNIQUE,
    username VARCAR NOT NULL UNIQUE,
    password VARCHAR NOT NULL,
    version INTEGER DEFAULT 0
);

CREATE TABLE groups (
    id INTEGER PRIMARY KEY NOT NULL,
    uid VARCHAR NOT NULL UNIQUE,
    name VARCHAR NOT NULL UNIQUE
);

CREATE TABLE group_users (
    users_id INTEGER NOT NULL,
    group_id INTEGER NOT NULL,
    PRIMARY KEY (users_id, group_id),
    FOREIGN KEY (users_id) REFERENCES users (id),
    FOREIGN KEY (group_id) REFERENCES groups (id)
);

CREATE TABLE authn_totp (
    users_id INTEGER PRIMARY KEY NOT NULL,
    algo SMALLINT NOT NULL,
    step INTEGER NOT NULL,
    digits INTEGER NOT NULL,
    secret BLOB NOT NULL,
    FOREIGN KEY (users_id) REFERENCES users (id)
);

CREATE TABLE authn_sessions (
    token BLOB PRIMARY KEY NOT NULL,
    users_id INTEGER NOT NULL,
    dropped BOOLEAN NOT NULL DEFAULT FALSE,
    issued_on INTEGER NOT NULL,
    expires_on INTEGER NOT NULL,
    authenticated BOOLEAN NOT NULL DEFAULT FALSE,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    FOREIGN KEY (users_id) REFERENCES users (id)
);

CREATE TABLE authz_roles (
    id INTEGER PRIMARY KEY NOT NULL,
    uid VARCHAR NOT NULL UNIQUE,
    name VARCHAR NOT NULL UNIQUE
);

CREATE TABLE authz_permissions (
    role_id INTEGER NOT NULL,
    scope VARCHAR NOT NULL,
    ability VARCHAR NOT NULL,
    PRIMARY KEY (role_id, scope, ability),
    FOREIGN KEY (role_id) REFERENCES authz_roles (id)
);

CREATE TABLE user_roles (
    users_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    PRIMARY KEY (users_id, role_id),
    FOREIGN KEY (users_id) REFERENCES users (id),
    FOREIGN KEY (role_id) REFERENCES authz_roles (id)
);

CREATE TABLE group_roles (
    group_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    PRIMARY KEY (group_id, role_id),
    FOREIGN KEY (group_id) REFERENCES groups (id),
    FOREIGN KEY (role_id) REFERENCES authz_roles (id)
);

CREATE TABLE journals (
    id INTEGER PRIMARY KEY NOT NULL,
    uid TEXT NOT NULL UNIQUE,
    users_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created DATETIME NOT NULL,
    updated DATETIME,
    UNIQUE (users_id, name),
    FOREIGN KEY (users_id) REFERENCES users (id)
);

CREATE TABLE entries (
    id INTEGER PRIMARY KEY NOT NULL,
    uid TEXT NOT NULL UNIQUE,
    journals_id INTEGER NOT NULL,
    users_id INTEGER NOT NULL,
    entry_date DATE NOT NULL,
    title TEXT,
    contents TEXT,
    created DATETIME NOT NULL,
    updated DATETIME,
    UNIQUE (users_id, entry_date),
    FOREIGN KEY (journals_id) REFERENCES journals (id),
    FOREIGN KEY (users_id) REFERENCES users (id)
);

CREATE TABLE entry_tags (
    entries_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT,
    created DATETIME NOT NULL,
    updated DATETIME,
    PRIMARY KEY (entries_id, key),
    FOREIGN KEY (entries_id) REFERENCES entries (id)
);

CREATE TABLE file_entries (
    id INTEGER PRIMARY KEY NOT NULL,
    uid TEXT NOT NULL UNIQUE,
    entries_id INTEGER NOT NULL,
    type INTEGER NOT NULL DEFAULT 0,
    name TEXT,
    mime_type TEXT NOT NULL,
    mime_subtype TEXT NOT NULL,
    mime_parameter TEXT,
    size INTEGER NOT NULL DEFAULT 0,
    created DATETIME NOT NULL,
    updated DATETIME,
    FOREIGN KEY (entries_id) REFERENCES entries (id)
);
