// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "item_kind"))]
    pub struct ItemKind;
}

diesel::table! {
    aliases (ean) {
        ean -> Varchar,
        alias_for -> Varchar,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ItemKind;

    items (id) {
        id -> Int4,
        name -> Varchar,
        kind -> ItemKind,
        ean -> Nullable<Varchar>,
    }
}

diesel::table! {
    stock (id) {
        id -> Int4,
        item_id -> Int4,
        added_dt -> Timestamptz,
        opened_dt -> Nullable<Timestamptz>,
        removed_dt -> Nullable<Timestamptz>,
    }
}

diesel::joinable!(stock -> items (item_id));

diesel::allow_tables_to_appear_in_same_query!(aliases, items, stock,);
