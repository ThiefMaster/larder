use anyhow::Result;
use brother_ql::{
    connection::{PrinterConnection, UsbConnection, UsbConnectionInfo},
    media::Media,
    printjob::PrintJobBuilder,
};
use datamatrix::{DataMatrix, SymbolList, placement::PathSegment};
use derive_typst_intoval::{IntoDict, IntoValue};
use image::DynamicImage;
use std::{
    fmt::Write,
    sync::{Arc, OnceLock},
};
use typst::foundations::{Bytes, Datetime, IntoValue};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt};
use typst::{diag::FileResult, foundations::Dict};
use typst_kit::fonts::{FontSearcher, FontSlot};

use crate::models::{Item, Stock};

#[allow(clippy::type_complexity)]
static FONT_DATA: OnceLock<(
    LazyHash<FontBook>,
    Arc<Vec<FontSlot>>,
    usize,
    Arc<Vec<Font>>,
)> = OnceLock::new();

pub struct LabelContent {
    pub name: String,
    pub date: String,
    pub code: String,
}

impl LabelContent {
    pub fn from_item_stock(item: &Item, stock: &Stock) -> Self {
        Self {
            name: item.name.clone(),
            date: stock.added_dt.date_naive().format("%m/%y").to_string(),
            code: format!("~{}|{}~", stock.item_id, stock.id),
        }
    }

    #[allow(unused)]
    pub fn new(name: &str, code: &str, date: &str) -> Self {
        Self {
            name: name.to_string(),
            date: date.to_string(),
            code: code.to_string(),
        }
    }
}

pub fn print_custom_item_labels(labels: &[LabelContent]) -> Result<()> {
    let info = UsbConnectionInfo::discover()?.ok_or_else(|| anyhow::anyhow!("No printer found"))?;
    let images: Vec<_> = labels
        .iter()
        .map(|content| {
            println!(
                "  generating label: code={} name='{}' date={}",
                content.code, content.name, content.date
            );
            generate_label(&content.name, &content.code, &content.date)
        })
        .collect();
    let mut conn = UsbConnection::open(info)?;
    println!("  printing {} labels", images.len());
    let mut it = images.into_iter();
    let job = PrintJobBuilder::new(Media::C62)
        .add_label(it.next().expect("Added at least one stock item"))
        .add_labels(it)
        .build()?;
    conn.print(job)?;
    Ok(())
}

fn generate_code_svg(code: &str) -> String {
    let bitmap = DataMatrix::encode(code.as_bytes(), SymbolList::default().enforce_square())
        .expect("Generating barcode should never fail")
        .bitmap();

    let mut svg: String = format!(
        concat!(
            r#"<?xml version="1.0"?>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
            r#"<path fill-rule="evenodd" d="M0,0"#,
        ),
        bitmap.width(),
        bitmap.height()
    )
    .to_owned();
    for part in bitmap.path() {
        match part {
            PathSegment::Horizontal(n) => write!(svg, "h{}", n),
            PathSegment::Vertical(n) => write!(svg, "v{}", n),
            PathSegment::Move(dx, dy) => write!(svg, "m{},{}", dx, dy),
            PathSegment::Close => write!(svg, "z"),
        }
        .expect("Writing to string should never fail");
    }
    svg.push_str(r#""/></svg>"#);
    svg
}

fn generate_label(name: &str, code: &str, date: &str) -> DynamicImage {
    let svg = generate_code_svg(code);

    let inputs = LabelInput {
        width: 696,
        height: 200,
        name: name.to_string(),
        date: date.to_string(),
        code: Bytes::from_string(svg),
    };
    let world = TypstWrapperWorld::new(include_str!("../typst/label.typ"), inputs.into_dict());

    let document: PagedDocument = typst::compile(&world)
        .output
        .map_err(|err| anyhow::anyhow!(format!("Typst compilation failed: {err:?}")))
        .unwrap();

    let pages: Vec<_> = document.pages.iter().collect();
    let page = pages
        .first()
        .ok_or_else(|| anyhow::anyhow!("Compiled document has no pages".to_string()))
        .unwrap();

    let pixmap = typst_render::render(page, 1.0);
    let buf = pixmap
        .encode_png()
        .map_err(|err| anyhow::anyhow!(format!("PNG encoding failed: {err}")))
        .unwrap();

    image::load_from_memory(&buf).unwrap()
}

#[derive(Debug, Clone, IntoValue, IntoDict)]
struct LabelInput {
    width: u16,
    height: u16,
    name: String,
    date: String,
    code: Bytes,
}

// The typst integration is based on the example from the brother_ql library:
// https://github.com/mkienitz/brother_ql/blob/main/crates/brother_ql/src/test_labels.rs
struct TypstWrapperWorld {
    /// The content of a source.
    source: Source,
    /// The standard library.
    library: LazyHash<Library>,
    /// Metadata about all known fonts.
    book: LazyHash<FontBook>,
    /// Shared reference to font data (Arc allows cheap cloning)
    fonts: Arc<Vec<FontSlot>>,
    /// Index at which custom fonts start
    custom_font_offset: usize,
    /// Custom fonts
    custom_fonts: Arc<Vec<Font>>,
}

impl TypstWrapperWorld {
    fn new(source: &str, inputs: Dict) -> Self {
        let (book, fonts, custom_font_offset, custom_fonts) = FONT_DATA.get_or_init(|| {
            let mut fonts = FontSearcher::new().include_system_fonts(false).search();
            // Add custom embedded font. This is super awful because lots of important parts are
            // private and thus need to be worked around (e.g. getting the number of fonts already
            // in the font book)
            let mut offset = 0;
            loop {
                if fonts.book.info(offset).is_none() {
                    break;
                }
                offset += 1;
            }
            let mut custom_fonts = Vec::new();
            for font in Font::iter(Bytes::new(include_bytes!(
                "../typst/LiberationSans-Regular.ttf"
            ))) {
                fonts.book.push(font.info().clone());
                custom_fonts.push(font);
            }
            (
                LazyHash::new(fonts.book),
                Arc::new(fonts.fonts),
                offset,
                Arc::new(custom_fonts),
            )
        });
        Self {
            source: Source::detached(source),
            library: LazyHash::new(Library::builder().with_inputs(inputs).build()),
            book: book.clone(),
            fonts: Arc::clone(fonts),
            custom_font_offset: *custom_font_offset,
            custom_fonts: Arc::clone(custom_fonts),
        }
    }
}

impl typst::World for TypstWrapperWorld {
    /// Standard library.
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    /// Metadata about all known Books.
    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    /// Accessing the main source file.
    fn main(&self) -> FileId {
        self.source.id()
    }

    /// Accessing a specified source file (based on `FileId`).
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.source.id() {
            Ok(self.source.clone())
        } else {
            panic!("Not implemented (nor needed)!")
        }
    }

    /// Accessing a specified file (non-file).
    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        panic!("Not implemented (nor needed)!")
    }

    /// Accessing a specified font per index of font book.
    fn font(&self, id: usize) -> Option<Font> {
        if id >= self.custom_font_offset {
            self.custom_fonts.get(id - self.custom_font_offset).cloned()
        } else {
            self.fonts[id].get()
        }
    }

    /// Get the current date.
    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}
