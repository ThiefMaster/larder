create table stock (
    id serial primary key,
    item_id int not null references items(id),
    added_dt timestamp not null default now(),
    opened_dt timestamp,
    removed_dt timestamp
);
