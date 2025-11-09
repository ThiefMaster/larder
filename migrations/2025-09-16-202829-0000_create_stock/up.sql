create table stock (
    id serial primary key,
    item_id int not null references items(id) on delete cascade,
    added_dt timestamptz not null default now(),
    opened_dt timestamptz,
    removed_dt timestamptz
);
