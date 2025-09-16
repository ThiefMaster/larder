use diesel::prelude::*;
use diesel::sql_types::Text;

use crate::schema::{aliases, items, stock};
use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::{deserialize, serialize};
use std::io::Write;
use std::time::SystemTime;

#[derive(Debug, Clone, FromSqlRow, AsExpression, PartialEq, Eq)]
#[diesel(sql_type = crate::schema::sql_types::ItemKind)]
pub enum ItemKind {
    Bought,
    Custom,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = items)]
#[allow(dead_code)]
pub struct Item {
    pub id: i32,
    pub name: String,
    pub kind: ItemKind,
    pub ean: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = items)]
pub struct NewItem<'a> {
    pub name: &'a str,
    pub kind: ItemKind,
    pub ean: Option<&'a str>,
}

#[derive(Debug, Queryable, Selectable, Insertable)]
#[diesel(table_name = aliases)]
#[allow(dead_code)]
pub struct Alias {
    pub ean: String,
    pub alias_for: String,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = stock)]
#[allow(dead_code)]
pub struct Stock {
    pub id: i32,
    pub item_id: i32,
    pub added_dt: SystemTime,
    pub opened_dt: Option<SystemTime>,
    pub removed_dt: Option<SystemTime>,
}

impl ToSql<crate::schema::sql_types::ItemKind, Pg> for ItemKind {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        match *self {
            ItemKind::Bought => out.write_all(b"bought")?,
            ItemKind::Custom => out.write_all(b"custom")?,
        }
        Ok(IsNull::No)
    }
}

impl FromSql<crate::schema::sql_types::ItemKind, Pg> for ItemKind {
    fn from_sql(bytes: PgValue) -> deserialize::Result<Self> {
        match bytes.as_bytes() {
            b"bought" => Ok(ItemKind::Bought),
            b"custom" => Ok(ItemKind::Custom),
            _ => Err(format!(
                "Unrecognized enum variant: {:?}",
                String::from_utf8_lossy(bytes.as_bytes())
            )
            .into()),
        }
    }
}

define_sql_function!(fn lower(x: Text) -> Text);
