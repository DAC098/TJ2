alter table user_invites
    add column role_id bigint references authz_roles (id),
    add column groups_id bigint references groups (id),
    drop column name;
