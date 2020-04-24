extern crate wasm_bindgen;
extern crate cfg_if;
#[macro_use]
extern crate serde_derive;

use cfg_if::cfg_if;
use std::io::{self, Cursor, Read};
use std::error::Error;
use std::fmt;
use image;
use image::{ColorType, ImageEncoder};
use image::imageops;
use jpeg_decoder;
use jpeg_decoder::PixelFormat;
use image::jpeg::JPEGEncoder;
use exif::{Tag, In};
use wasm_bindgen::JsValue;

use wasm_bindgen::prelude::*;

cfg_if! {
   // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
   if #[cfg(feature = "wee_alloc")] {
       extern crate wee_alloc;
       #[global_allocator]
       static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
   }
}

#[derive(Debug, Clone)]
struct NailError {
    message: String
}

impl Error for NailError {
    fn description(&self) -> &str {
       return &self.message.as_str()
    }

    fn cause(&self) -> Option<&(dyn Error)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl fmt::Display for NailError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No thumbnail")
    }
}

fn get_orientation(input: &[u8]) -> Result<u32, Box<dyn Error>> {
    let data = exif::Reader::new().read_from_container(&mut io::Cursor::new(input))?;
    match data.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        Some(res) => Ok(res.value.get_uint(0).unwrap_or(0)),
        _ => Ok(0)
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[derive(Serialize)]
pub struct JSErr {
    message: String,
    source: String
}

// TODO: Can we use a macro for this boilerplate? Result<_, Box<dyn Error>> to Result<_, JsValue>
#[wasm_bindgen]
pub fn scale_and_orient(input: &[u8], max_dim: u32) -> Result<Vec<u8>, JsValue> {
    match _scale_and_orient(&input, max_dim) {
        Ok(val) => Ok(val),
        Err(e) => Err(JsValue::from_serde(&JSErr {
            message: e.to_string(),
            source: "nail-salon".into()
        }).unwrap())
    }
}

fn _scale_and_orient(input: &[u8], max_dim: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let ouput_fmt = image::ImageFormat::Jpeg;
    let orientation = get_orientation(input);
    let img = image::load_from_memory(&input)?;

    let thumb = img.thumbnail(max_dim, max_dim);
    let thumb= match orientation {
        // Reference: https://www.daveperrett.com/articles/2012/07/28/exif-orientation-handling-is-a-ghetto/
        // Reference: http://sylvana.net/jpegcrop/exif_orientation.html
        Ok(2) => thumb.fliph(),
        Ok(3) => thumb.rotate180(),
        Ok(4) => thumb.flipv(),
        Ok(5) => thumb.rotate90().fliph(),
        Ok(6) => thumb.rotate90(),
        Ok(7) => thumb.rotate90().flipv(),
        Ok(8) => thumb.rotate270(),
        Err(err) => return Err(err),
        _ => thumb
    };

    let mut out: Vec<u8> = Vec::new();
    thumb.write_to(&mut out, ouput_fmt).expect("Failed to write output");
    return Ok(out);
}

#[wasm_bindgen]
pub fn fast_scale_and_orient(input: &[u8], max_dim: u16) -> Result<Vec<u8>, JsValue> {
    match _fast_scale_and_orient(input, max_dim) {
        Ok(val) => Ok(val),
        Err(e) => Err(JsValue::from_serde(&JSErr {
            message: e.to_string(),
            source: "nail-salon".into()
        }).unwrap())
    }
}

fn _fast_scale_and_orient(input: &[u8], max_dim: u16) -> Result<Vec<u8>, Box<dyn Error>> {
    let orientation = get_orientation(input).unwrap_or(0);

    let mut decoder = jpeg_decoder::Decoder::new(io::Cursor::new(input));
    decoder.scale(max_dim, max_dim)?;

    let metadata = decoder.info().unwrap();

    // We're only optimizing RGB for now
    match metadata.pixel_format {
        PixelFormat::RGB24 => (),
        // We can add support for these CMYK, L8 in the future if needed.
        _ => return Err(NailError{ message: "Unsupported format".into()}.into())
    };

    let img_buf = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
        metadata.width as u32,
        metadata.height as u32,
        decoder.decode()?
    ).unwrap();

    let img_buf = match orientation {
        2 => imageops::flip_horizontal(&img_buf),
        3 => imageops::rotate180(&img_buf),
        4 => imageops::flip_vertical(&img_buf),
        5 => imageops::flip_horizontal(&imageops::rotate90(&img_buf)),
        6 => imageops::rotate90(&img_buf),
        7 => imageops::flip_vertical(&imageops::rotate90(&img_buf)),
        8 => imageops::rotate270(&img_buf),
        _ => img_buf
    };

    let out: Vec<u8> = Vec::new();
    let mut curs = io::Cursor::new(out);

    let height = if orientation >= 5 { metadata.width } else {metadata.height };
    let width = if orientation >= 5 { metadata.height } else {metadata.width };

    let enc = JPEGEncoder::new_with_quality(&mut curs, 90);

    enc.write_image(
        &img_buf.as_ref(),
        width as u32,
        height as u32,
        ColorType::Rgb8)?;

    return Ok(curs.into_inner());
}


#[wasm_bindgen]
pub fn embedded_thumbnail(input: &[u8]) -> Result<Vec<u8>, JsValue> {
    match _embedded_thumbnail(input) {
        Ok(val) => Ok(val),
        Err(e) => Err(JsValue::from_serde(&JSErr {
            message: e.to_string(),
            source: "nail-salon".into()
        }).unwrap())
    }
}

fn _embedded_thumbnail(input: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut curs = Cursor::new(input);

    let meta = exif::Reader::new().read_from_container( &mut curs)?;

    let thumb_compression = match meta.get_field(Tag::Compression, In::THUMBNAIL) {
        Some(field) => field.value.get_uint(0),
        _ => return Err(NailError {message: "No thumbnail".into()}.into())
    };

    match thumb_compression {
        Some(6) => (),
        _ => return Err(NailError {message: "Unsupported thumbnail compression".into()}.into())
    };
    let offset = meta.get_field(Tag::JPEGInterchangeFormat, In::THUMBNAIL).unwrap()
        .value.get_uint(0).unwrap();

    let length = meta.get_field(Tag::JPEGInterchangeFormatLength, In::THUMBNAIL).unwrap()
        .value.get_uint(0).unwrap();

    let mut curs = Cursor::new(input);
    curs.set_position((offset + 12) as u64);

    let mut out = Vec::new();
    let mut curs = curs.clone();
    curs.set_position((offset + 12) as u64);
    curs.take((offset + 12 + length) as u64).read_to_end(&mut out)?;
    return Ok(out);
}

#[derive(Serialize)]
pub struct ExifItem {
    ifd: u16,
    tag: u16,
    desc: String,
    value: String,
}

#[wasm_bindgen(typescript_custom_section)]
const EXIF_DATA_FORMAT: &'static str = r#"
interface ExifItem {
    ifd: number;
    tag: number;
    desc: string;
    value: string;
}

interface ImageMetadata {
    primary: ExifItem[];
    thumbnail: ExifItem[];
}
"#;

#[wasm_bindgen(typescript_type = "EXIF_DATA_FORMAT")]
#[derive(Serialize)]
pub struct ImageMetadata {
    primary: Vec<ExifItem>,
    thumbnail: Vec<ExifItem>
}

/// Extract image metadata
#[wasm_bindgen(typescript_type = "(input: Uint8Array) => ImageMetadata")]
pub fn exif_data(input: &[u8]) -> Result<JsValue, JsValue> {
    match _exif_data(input) {
        Ok(val) => Ok(JsValue::from_serde(&val).unwrap()),
        Err(e) => return Err(JsValue::from_serde(&JSErr {
            message: e.to_string(),
            source: "nail-salon".into()
        }).unwrap())
    }
}

fn _exif_data(input: &[u8]) -> Result<ImageMetadata, Box<dyn Error>> {
    let mut curs = Cursor::new(input);
    let exif_data = exif::Reader::new().read_from_container(&mut curs)?;

    let mut primary: Vec<ExifItem>  =  Vec::new();
    let mut thumbnail: Vec<ExifItem>  =  Vec::new();

    for field in exif_data.fields() {
        if Tag::MakerNote == field.tag {
            continue;
        }

        let item = ExifItem {
            ifd: field.ifd_num.index(),
            tag: field.tag.number(),
            desc: field.tag.description().unwrap_or("").to_string(),
            value: field.display_value().to_string()
        };

        match field.ifd_num {
            In::THUMBNAIL => thumbnail.push(item),
            _ => primary.push(item),
            // _ => panic!() // for now
        }
    }

    return Ok(ImageMetadata{primary, thumbnail});
}
