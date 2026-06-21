use anyhow::Result;
use larder::labels::generate_label;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let path = args.get(1).expect("path argument missing");
    let name = args.get(2).expect("name argument missing");
    let date = chrono::Local::now()
        .date_naive()
        .format("%m/%y")
        .to_string();
    let label = generate_label(name, "XXXX", &date);
    label
        .save_with_format(path, image::ImageFormat::Png)
        .unwrap();
    Ok(())
}
