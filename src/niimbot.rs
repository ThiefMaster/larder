use anyhow::Result;
use reqwest::{
    StatusCode,
    blocking::{Client, ClientBuilder},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::Serialize_repr;
use serde_with::skip_serializing_none;

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(unused)]
enum PrintDirection {
    Left,
    Top,
}

#[derive(Debug, Serialize_repr)]
#[repr(u8)]
#[allow(unused)]
enum LabelType {
    Invalid = 0,
    WithGaps = 1,
    Black = 2,
    Continuous = 3,
    Perforated = 4,
    Transparent = 5,
    PvcTag = 6,
    BlackMarkGap = 10,
    HeatShrinkTube = 11,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(unused)]
enum ImagePosition {
    #[serde(rename(serialize = "centre"))]
    Center,
    Top,
    #[serde(rename(serialize = "right top"))]
    RightTop,
    Right,
    #[serde(rename(serialize = "right bottom"))]
    RightBottom,
    Bottom,
    #[serde(rename(serialize = "left bottom"))]
    LeftBottom,
    Left,
    #[serde(rename(serialize = "left top"))]
    LeftTop,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(unused)]
enum ImageFit {
    Contain,
    Cover,
    Fill,
    Inside,
    Outside,
}

#[skip_serializing_none]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
struct PrintJob {
    print_direction: Option<PrintDirection>,
    print_task: Option<String>,
    quantity: u64,
    label_type: LabelType,
    density: u8,
    image_base64: Option<String>,
    image_url: Option<String>,
    label_width: Option<u64>,
    label_height: Option<u64>,
    threshold: u8,
    image_position: ImagePosition,
    image_fit: ImageFit,
}

impl Default for PrintJob {
    fn default() -> Self {
        Self {
            print_direction: None,
            print_task: None,
            quantity: 1,
            label_type: LabelType::WithGaps,
            density: 3,
            image_base64: None,
            image_url: None,
            label_width: None,
            label_height: None,
            threshold: 128,
            image_position: ImagePosition::Center,
            image_fit: ImageFit::Contain,
        }
    }
}

#[derive(Debug, Serialize)]
struct ConnectRequest<'a> {
    transport: &'a str,
    address: &'a str,
}

#[derive(Debug, Deserialize)]
struct APIResponse<'a> {
    message: Option<&'a str>,
    error: Option<&'a str>,
}

pub fn print_label(image_base64: &str) -> Result<bool> {
    connect_printer()?;
    if !check_printer()? {
        return Ok(false);
    }

    let payload = PrintJob {
        image_base64: Some(image_base64.to_string()),
        ..Default::default()
    };
    let req = build_http_client()
        .post("http://localhost:58000/print")
        .json(&payload);

    let http_resp = req.send()?;
    let status = http_resp.status();
    let text = http_resp.text()?;

    if !status.is_success() {
        anyhow::bail!("Printer print request failed ({status}): {text}");
    }
    let data: APIResponse = serde_json::from_str(&text)?;
    if data.message.is_none_or(|x| x != "Printed") {
        anyhow::bail!("Unexpected printer print response: {data:?}");
    }

    Ok(true)
}

fn connect_printer() -> Result<()> {
    let payload = ConnectRequest {
        transport: "serial",
        address: "/dev/ttyACM0",
    };
    let req = build_http_client()
        .post("http://localhost:58000/connect")
        .json(&payload);

    let http_resp = req.send()?;
    let status = http_resp.status();
    let text = http_resp.text()?;
    if !status.is_success() && status != StatusCode::BAD_REQUEST {
        anyhow::bail!("Printer connect request failed ({status}): {text}");
    }
    let data: APIResponse = serde_json::from_str(&text)?;
    if data.message.is_some_and(|x| x != "Connected")
        || data.error.is_some_and(|x| x != "Already connected")
    {
        anyhow::bail!("Unexpected printer connect response: {data:?}");
    }
    Ok(())
}

fn check_printer() -> Result<bool> {
    let req = build_http_client().get("http://localhost:58000/info");
    let http_resp = req.send()?;
    let status = http_resp.status();
    let text = http_resp.text()?;
    if !status.is_success() {
        return Ok(false);
    }
    let data: Value = serde_json::from_str(&text)?;
    // if we're connecting, we get a success response w/ empty printerInfo, but
    // modelMetadata is only present when we actually have details about the printer
    Ok(data.get("modelMetadata").is_some())
}

fn build_http_client() -> Client {
    ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build")
}
