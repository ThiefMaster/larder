use anyhow::Result;
use base64::Engine;
use chrono::NaiveDate;
use datamatrix::{DataMatrix, SymbolList, placement::PathSegment};
use resvg::{tiny_skia, usvg};
use std::{borrow::Cow, fmt::Write, sync::Arc};

use crate::{models::Item, niimbot::print_label};

pub fn print_custom_item_label(item: &Item, code: &str, date: &NaiveDate) -> Result<bool> {
    let (name1, name2) = split_name(&item.name);
    let date = date.format("%m/%y").to_string();
    let image_base64 = generate_label(code, &name1, &name2, &date);
    println!("  printing label: code={code} name1='{name1}' name2='{name2}' date={date}");
    print_label(&image_base64)
}

fn split_name(name: &str) -> (String, String) {
    let parts = textwrap::wrap(
        name,
        textwrap::Options::new(20).wrap_algorithm(textwrap::WrapAlgorithm::FirstFit),
    );

    let name1 = (*parts[0]).to_string();
    let rest = parts
        .into_iter()
        .skip(1)
        .collect::<Vec<Cow<str>>>()
        .join(" ");
    (name1, rest)
}

fn generate_label(code: &str, name1: &str, name2: &str, date: &str) -> String {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut()
        .load_font_source(usvg::fontdb::Source::Binary(Arc::new(include_bytes!(
            "LiberationSans-Regular.ttf"
        ))));

    let bitmap = DataMatrix::encode(code.as_bytes(), SymbolList::default().enforce_square())
        .unwrap()
        .bitmap();
    let mut matrix_path: String =
        r#"<path transform="translate(40,110)" fill-rule="evenodd" d="M1,1"#.to_owned();
    for part in bitmap.path() {
        match part {
            PathSegment::Horizontal(n) => write!(matrix_path, "h{}", n * 7),
            PathSegment::Vertical(n) => write!(matrix_path, "v{}", n * 7),
            PathSegment::Move(dx, dy) => write!(matrix_path, "m{},{}", dx * 7, dy * 7),
            PathSegment::Close => write!(matrix_path, "z"),
        }
        .unwrap();
    }
    matrix_path.push_str(r#""/>"#);

    let svg_data = include_str!("label.svg")
        .replace("$MATRIX$", &matrix_path)
        .replace("$DATE$", date)
        .replace("$NAME1$", name1)
        .replace("$NAME2$", name2);

    let svg_tree = usvg::Tree::from_data(svg_data.as_bytes(), &opt).unwrap();

    let pixmap_size = svg_tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();
    resvg::render(
        &svg_tree,
        tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    let mut image_base64 = String::new();
    base64::engine::general_purpose::STANDARD
        .encode_string(pixmap.encode_png().unwrap(), &mut image_base64);
    image_base64
}
