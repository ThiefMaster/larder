create type item_kind as enum ('custom', 'bought');

create table items (
    id serial primary key,
    name varchar not null,
    kind item_kind not null,
    ean varchar,
    unique(ean)
);

create unique index on items (lower(name));
