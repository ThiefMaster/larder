use anyhow::Result;
use diesel::prelude::*;
use std::env;

use crate::models::{Alias, Item, ItemKind, NewItem, lower};

fn connect_db() -> Result<PgConnection> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .map_err(|err| anyhow::anyhow!("Error connecting to {database_url}: {err}"))
}

pub fn query_item_by_ean(barcode_ean: &str) -> Result<Option<Item>> {
    use crate::schema::items::dsl::*;

    let mut conn = &mut connect_db()?;
    let barcode_ean =
        query_ean_by_alias(&mut conn, &barcode_ean)?.unwrap_or(barcode_ean.to_string());

    items
        .filter(ean.eq(barcode_ean.as_str()))
        .select(Item::as_select())
        .first(conn)
        .optional()
        .map_err(|err| anyhow::anyhow!("Could not load item {barcode_ean}: {err}"))
}

fn query_ean_by_alias(conn: &mut PgConnection, alias_ean: &str) -> Result<Option<String>> {
    use crate::schema::aliases::dsl::*;

    aliases
        .find(alias_ean)
        .select(Alias::as_select())
        .first(conn)
        .optional()
        .map_err(|err| anyhow::anyhow!("Could not load alias for {alias_ean}: {err}"))
        .map(|opt| opt.map(|a| a.alias_for))
}

pub fn query_item_by_name(ci_name: &str) -> Result<Option<Item>> {
    use crate::schema::items::dsl::*;

    let conn = &mut connect_db()?;
    items
        .filter(lower(name).eq(lower(ci_name)))
        .select(Item::as_select())
        .first(conn)
        .optional()
        .map_err(|err| anyhow::anyhow!("Could not check for similar item: {err}"))
}

pub fn create_item(barcode_ean: &str, name: &str) -> Result<Item> {
    use crate::schema::items;

    let new_item = NewItem {
        name,
        kind: ItemKind::Bought,
        ean: Some(barcode_ean),
    };

    let conn = &mut connect_db()?;
    diesel::insert_into(items::table)
        .values(&new_item)
        .returning(Item::as_returning())
        .get_result(conn)
        .map_err(|err| anyhow::anyhow!("Could not insert item {new_item:?}: {err}"))
}

pub fn create_alias(alias_ean: &str, item_ean: &str) -> Result<Alias> {
    use crate::schema::aliases;

    let new_alias = Alias {
        ean: alias_ean.to_string(),
        alias_for: item_ean.to_string(),
    };

    let conn = &mut connect_db()?;
    diesel::insert_into(aliases::table)
        .values(&new_alias)
        .returning(Alias::as_returning())
        .get_result(conn)
        .map_err(|err| anyhow::anyhow!("Could not insert alias {new_alias:?}: {err}"))
}
