use crate::db::{
    add_to_stock, connect_db, create_alias, create_item, finish_from_stock, open_from_stock,
    query_item_by_ean, query_item_by_id, query_item_by_name, remove_from_stock,
    search_custom_items_by_name,
};
use crate::keyinput::read_input;
use crate::labels::{LabelContent, print_custom_item_labels};
use crate::models::{Item, Stock};
use anyhow::Result;
use diesel::Connection;
use dotenvy::dotenv;
use openfoodfacts::{self as off, Output};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use std::{str::FromStr, sync::mpsc, thread};
use termios::{TCIOFLUSH, tcflush};
use text_io::{read, try_scan};

mod db;
mod keyinput;
mod labels;
mod models;
mod schema;
// mod web;

static IDLE_TIMEOUT: u64 = 120;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ScanOp {
    None,
    Register,
    Add,
    Remove,
    Open,
    Finish,
}

impl FromStr for ScanOp {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "???" => Ok(ScanOp::None),
            "+++" => Ok(ScanOp::Register),
            ">>>" => Ok(ScanOp::Add),
            "<<<" => Ok(ScanOp::Remove),
            "///" => Ok(ScanOp::Open),
            "</<" => Ok(ScanOp::Finish),
            // ~+~ => create custom: handled separately, it's an action and not an op that affects later scans
            _ => Err(()),
        }
    }
}

fn main() -> Result<()> {
    dotenv().ok();
    let device_path = std::env::args()
        .nth(1)
        .unwrap_or(String::from("/dev/input/event0"));

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || read_input(&device_path, tx));

    let mut op = ScanOp::None;
    let idle_timeout = Duration::from_secs(IDLE_TIMEOUT);
    loop {
        match rx.recv_timeout(idle_timeout) {
            Ok(line) => {
                println!("recv: '{line}'");
                if let Ok(new_op) = ScanOp::from_str(&line) {
                    if new_op != op {
                        println!("scan op changed: {op:?} -> {new_op:?}");
                        op = new_op;
                    }
                } else if line == "~+~" {
                    if let Err(err) = create_custom() {
                        println!("creating custom item failed: {err}");
                    }
                } else if let Some((item_id, stock_id)) = parse_custom_code(&line) {
                    if let Err(err) = remove_custom(item_id, stock_id) {
                        println!("removing custom item from stock failed: {err}");
                    }
                } else if let Err(err) = scanned(op, &line) {
                    println!("processing scan {line} failed: {err}");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if op != ScanOp::None {
                    println!("scan op reset: {op:?} -> None");
                    op = ScanOp::None;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("Input channel disconnected");
            }
        }
    }
}

fn parse_custom_code(line: &str) -> Option<(i32, i32)> {
    // AFAICT, `try_read!` does not support more than one placeholder, and
    // unfortunately `try_scan!` includes a hardcoded `?` for error handling,
    // so we need this extra function to get the Result which we can then
    // convert to na Option
    let inner = || -> Result<(i32, i32)> {
        let (item_id, stock_id): (i32, i32);
        try_scan!(line.bytes() => "~{}|{}~", item_id, stock_id);
        Ok((item_id, stock_id))
    };
    inner().ok()
}

fn create_custom() -> Result<()> {
    println!("Adding custom item");
    print!("  enter name: ");
    tcflush(0, TCIOFLUSH).unwrap();
    let name: String = read!("{}\n");
    if name.is_empty() {
        println!();
        anyhow::bail!("no name provided");
    }
    let candidates = search_custom_items_by_name(&name)?;
    let item = if let [cand] = candidates.as_slice()
        && cand.name.to_lowercase() == name.to_lowercase()
    {
        println!("  found existing item");
        candidates[0].to_owned()
    } else if !candidates.is_empty() {
        println!("  found {} existing items:", candidates.len());
        for (i, item) in candidates.iter().enumerate() {
            println!("  - [{}] {}", i + 1, item.name);
        }
        print!("  enter number or leave empty to create new item, X to cancel: ");
        loop {
            let choice: String = read!("{}\n");
            if choice.is_empty() {
                let item = create_item(None, &name)?;
                println!("  created {item:?}");
                break item;
            } else if choice.to_lowercase() == "x" {
                anyhow::bail!("aborted");
            } else {
                let idx = match choice.parse::<usize>() {
                    Err(err) => {
                        print!("  invalid input ({err}), try again: ");
                        continue;
                    }
                    Ok(0) => {
                        print!("  invalid index, try again: ");
                        continue;
                    }
                    Ok(idx) => idx,
                };
                match candidates.get(idx - 1) {
                    Some(item) => break item.to_owned(),
                    None => {
                        print!("  invalid index, try again: ");
                        continue;
                    }
                }
            };
        }
    } else {
        print!("  no existing item found, create new? [Y/n] ");
        tcflush(0, TCIOFLUSH).unwrap();
        let s: String = read!("{}\n");
        if !s.is_empty() && s.to_lowercase() != "y" {
            anyhow::bail!("aborted");
        }
        let item = create_item(None, &name)?;
        println!("  created {item:?}");
        item
    };
    print!("  enter count [1]: ");
    let count = loop {
        let resp: String = read!("{}\n");
        if resp.is_empty() {
            break 1;
        } else {
            match resp.parse::<u8>() {
                Err(err) => {
                    println!("  invalid input ({err}), try again: ");
                    continue;
                }
                Ok(0) => {
                    anyhow::bail!("nothing to add to stock");
                }
                Ok(count) => break count,
            }
        };
    };
    let mut conn = connect_db()?;
    conn.transaction::<_, anyhow::Error, _>(|conn| {
        let mut labels = Vec::<LabelContent>::with_capacity(count.into());
        for i in 0..count {
            println!("  adding to stock [{}/{}]", i + 1, count);
            let stock = add_to_stock(&item, Some(conn))?;
            labels.push(LabelContent::from_item_stock(&item, &stock));
        }
        print_custom_item_labels(&labels)
    })?;
    Ok(())
}

fn remove_custom(item_id: i32, stock_id: i32) -> Result<()> {
    let item = match query_item_by_id(item_id)? {
        None => {
            println!("Cannot remove custom item {item_id}, not found");
            return Ok(());
        }
        Some(item) => item,
    };
    println!("Removing custom from stock: {}", item.name);
    match remove_from_stock(&item, Some(stock_id))? {
        Ok(_) => println!("  successful"),
        Err(err) => println!("  {err}"),
    }
    Ok(())
}

fn scanned(op: ScanOp, barcode: &str) -> Result<()> {
    let mut existing = query_item_by_ean(barcode)?;
    match op {
        ScanOp::None => {
            match existing {
                Some(item) => println!("Item found {item:?}"),
                None => println!("No such item: {barcode}"),
            };
        }
        ScanOp::Register => {
            register(barcode, existing)?;
        }
        ScanOp::Add => {
            if existing.is_none() {
                println!("Trying to add {barcode}, but no item found");
                existing = register(barcode, existing)?;
                if existing.is_none() {
                    println!("  no item added");
                    return Ok(());
                }
            }
            add(existing.unwrap())?;
        }
        ScanOp::Remove => {
            if existing.is_none() {
                println!("Cannot remove {barcode}, no item found");
                return Ok(());
            }
            remove(existing.unwrap())?;
        }
        ScanOp::Open => {
            if existing.is_none() {
                println!("Cannot open {barcode}, no item found");
                return Ok(());
            }
            open(existing.unwrap())?;
        }
        ScanOp::Finish => {
            if existing.is_none() {
                println!("Cannot finish {barcode}, no item found");
                return Ok(());
            }
            finish(existing.unwrap())?;
        }
    }
    Ok(())
}

fn add(item: Item) -> Result<Stock> {
    println!("Adding to stock: {}", item.name);
    let res = add_to_stock(&item, None);
    match res {
        Ok(_) => println!("  successful"),
        Err(ref err) => println!("  {err}"),
    }
    res
}

fn remove(item: Item) -> Result<()> {
    println!("Removing from stock: {}", item.name);
    match remove_from_stock(&item, None)? {
        Ok(_) => println!("  successful"),
        Err(err) => println!("  {err}"),
    }
    Ok(())
}

fn open(item: Item) -> Result<()> {
    println!("Opening: {}", item.name);
    match open_from_stock(&item)? {
        Ok(_) => println!("  successful"),
        Err(err) => println!("  {err}"),
    }
    Ok(())
}

fn finish(item: Item) -> Result<()> {
    println!("Finishing: {}", item.name);
    match finish_from_stock(&item)? {
        Ok(_) => println!("  successful"),
        Err(err) => println!("  {err}"),
    }
    Ok(())
}

fn register(barcode: &str, existing: Option<Item>) -> Result<Option<Item>> {
    println!("Registering {barcode}");
    if let Some(item) = existing {
        println!("  already registered ({})", item.name);
        return Ok(None);
    }
    println!("  looking up name via openfoodfacts");
    let name = lookup(barcode)?
        .map(|n| {
            println!(r#"  found "{n}""#);
            n.to_string()
        })
        .or_else(|| {
            print!("  nothing found, enter manually: ");
            tcflush(0, TCIOFLUSH).unwrap();
            let s: String = read!("{}\n");
            if s.is_empty() {
                println!();
                None
            } else {
                Some(s)
            }
        })
        .and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .ok_or(anyhow::anyhow!("no name provided"))?;

    if let Some(item) = query_item_by_name(&name)? {
        let conflict_ean = item
            .ean
            .clone()
            .ok_or_else(|| anyhow::anyhow!("name collision with custom item"))?;
        print!("  name collision with {conflict_ean} - create alias? [Y/n] ");
        tcflush(0, TCIOFLUSH).unwrap();
        let s: String = read!("{}\n");
        if !s.is_empty() && s.to_lowercase() != "y" {
            anyhow::bail!("Unresolved name conflict");
        }
        create_alias(barcode, &conflict_ean)?;
        println!("  alias created");
        return Ok(Some(item));
    }

    let item = create_item(Some(barcode), &name)?;
    println!("  created {item:?}");
    Ok(Some(item))
}

fn lookup(ean: &str) -> Result<Option<String>> {
    if ean == "4061463732958" {
        // wrong data in off, it's aldi kleenex and not bread...
        return Ok(None);
    }
    let client = off::v0().build().unwrap();
    let settings = Some(Output::new().fields("product_name,product_name_de"));
    let response = client
        .product(ean, settings)
        .map_err(|err| anyhow::anyhow!("Could not load product: {err}"))?;
    let data = json!(response.json::<HashMap::<String, Value>>()?);
    if data["status"].as_i64().unwrap_or(0) != 1 {
        return Ok(None);
    }
    data["product"]["product_name_de"]
        .as_str()
        .or(data["product"]["product_name"].as_str())
        .map(|n| Some(n.into()))
        .ok_or(anyhow::anyhow!("Product has no name"))
}
