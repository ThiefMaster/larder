use anyhow::Result;
use diesel::{dsl::now, prelude::*, sql_query, sql_types::Integer};
use std::env;

use crate::models::{Alias, Item, ItemKind, NewItem, Stock, lower};

fn connect_db() -> Result<PgConnection> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url)
        .map_err(|err| anyhow::anyhow!("Error connecting to {database_url}: {err}"))
}

pub fn query_item_by_ean(barcode_ean: &str) -> Result<Option<Item>> {
    use crate::schema::items::dsl::*;

    let conn = &mut connect_db()?;
    let barcode_ean = query_ean_by_alias(conn, barcode_ean)?.unwrap_or(barcode_ean.to_string());

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

pub fn query_item_by_id(id: i32) -> Result<Option<Item>> {
    use crate::schema::items::dsl::items;

    let conn = &mut connect_db()?;
    items
        .find(id)
        .select(Item::as_select())
        .first(conn)
        .optional()
        .map_err(|err| anyhow::anyhow!("Could not get item: {err}"))
}

pub fn search_custom_items_by_name(ci_name: &str) -> Result<Vec<Item>> {
    use crate::schema::items::dsl::*;

    let conn = &mut connect_db()?;
    items
        .filter(name.ilike(format!("%{ci_name}%")))
        .filter(kind.eq(ItemKind::Custom))
        .select(Item::as_select())
        .order(lower(name))
        .load(conn)
        .map_err(|err| anyhow::anyhow!("Could not query custom items: {err}"))
}

pub fn create_item(barcode_ean: Option<&str>, name: &str) -> Result<Item> {
    use crate::schema::items;

    let new_item = NewItem {
        name,
        kind: if barcode_ean.is_some() {
            ItemKind::Bought
        } else {
            ItemKind::Custom
        },
        ean: barcode_ean,
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

pub fn add_to_stock(item: &Item) -> Result<Stock> {
    use crate::schema::stock;
    use crate::schema::stock::dsl::*;

    let conn = &mut connect_db()?;
    diesel::insert_into(stock::table)
        .values(item_id.eq(item.id))
        .returning(Stock::as_returning())
        .get_result(conn)
        .map_err(|err| {
            anyhow::anyhow!(
                "Could not insert stock for {item_id:?}: {err}",
                item_id = item.id
            )
        })
}

pub fn remove_from_stock(item: &Item, stock_id: Option<i32>) -> Result<Result<()>> {
    use crate::schema::stock;
    use crate::schema::stock::dsl;

    let conn = &mut connect_db()?;
    let rows = match stock_id {
        None => sql_query(
            r#"
            with oldest as (
                select id
                from stock
                where item_id = $1 and opened_dt is null and removed_dt is null
                order by added_dt asc
                limit 1
            )
            update stock s
            set removed_dt = now()
            from oldest
            where s.id = oldest.id;
        "#,
        )
        .bind::<Integer, _>(item.id)
        .execute(conn)?,
        Some(stock_id) => diesel::update(stock::table)
            .filter(
                dsl::id
                    .eq(stock_id)
                    .and(dsl::item_id.eq(item.id))
                    .and(dsl::removed_dt.is_null()),
            )
            .set(dsl::removed_dt.eq(now))
            .execute(conn)?,
    };
    Ok(if rows > 0 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("item not in stock"))
    })
}

pub fn open_from_stock(item: &Item) -> Result<Result<()>> {
    use crate::schema::stock::dsl::*;
    use diesel::dsl::{exists, select};

    let conn = &mut connect_db()?;

    let already_open = select(exists(
        stock.filter(
            item_id
                .eq(item.id)
                .and(removed_dt.is_null())
                .and(opened_dt.is_not_null()),
        ),
    ))
    .get_result::<bool>(conn)?;
    if already_open {
        // TODO decide whether to allow having more than one open stock for an item
        return Ok(Err(anyhow::anyhow!("found open item in stock")));
    }

    let rows = sql_query(
        r#"
        with oldest as (
            select id
            from stock
            where item_id = $1 and opened_dt is null and removed_dt is null
            order by added_dt asc
            limit 1
        )
        update stock s
        set opened_dt = now()
        from oldest
        where s.id = oldest.id;
        "#,
    )
    .bind::<Integer, _>(item.id)
    .execute(conn)?;
    Ok(if rows > 0 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("item not in stock"))
    })
}

pub fn finish_from_stock(item: &Item) -> Result<Result<()>> {
    let conn = &mut connect_db()?;

    let rows = sql_query(
        r#"
        with oldest as (
            select id
            from stock
            where item_id = $1 and opened_dt is not null and removed_dt is null
            order by opened_dt asc
            limit 1
        )
        update stock s
        set removed_dt = now()
        from oldest
        where s.id = oldest.id;
        "#,
    )
    .bind::<Integer, _>(item.id)
    .execute(conn)?;
    Ok(if rows > 0 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("item not in stock or not opened"))
    })
}
