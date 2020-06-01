#[macro_use]
extern crate serde_derive;
use image::jpeg::JPEGEncoder;
use image::{imageops, GenericImageView};
use image::{ColorType, ImageEncoder};
use jpeg_decoder::PixelFormat;
use std::error::Error;
use std::fmt;
use std::io;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone)]
pub struct NailError {
    message: String,
}

impl Error for NailError {
    fn description(&self) -> &str {
        &self.message.as_str()
    }
}

impl fmt::Display for NailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NailError: {}", self.message)
    }
}

#[derive(Debug)]
pub enum MultiErr {
    NailError(NailError),
    JpegError(jpeg_decoder::Error),
    ImageError(image::error::ImageError),
    ExifErr(exif::Error),
}
impl From<NailError> for MultiErr {
    fn from(err: NailError) -> MultiErr {
        MultiErr::NailError(err)
    }
}
impl From<jpeg_decoder::Error> for MultiErr {
    fn from(err: jpeg_decoder::Error) -> MultiErr {
        MultiErr::JpegError(err)
    }
}

impl From<image::error::ImageError> for MultiErr {
    fn from(err: image::error::ImageError) -> MultiErr {
        MultiErr::ImageError(err)
    }
}

impl From<MultiErr> for JsValue {
    fn from(val: MultiErr) -> JsValue {
        JsValue::from_serde(&JSErr {
            message: format!("Error: {:?}", val),
            source: SOURCE.to_string(),
        })
        .unwrap()
    }
}

fn get_orientation(input: &[u8]) -> Result<u32, exif::Error> {
    let data = exif::Reader::new().read_from_container(&mut io::Cursor::new(&input))?;
    match data.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        Some(res) => Ok(res.value.get_uint(0).unwrap_or(0)),
        _ => Ok(0),
    }
}

#[derive(Serialize)]
pub struct JSErr {
    message: String,
    source: String,
}

const SOURCE: &str = "nail-salon";

// TODO: Can we create a macro for this boilerplate
#[wasm_bindgen]
pub fn scale_and_orient(input: &[u8], max_w: u16, max_h: u16) -> Result<Vec<u8>, JsValue> {
    match _scale_and_orient(&input, max_w, max_h) {
        Ok(val) => Ok(val),
        Err(e) => Err(e.into()),
    }
}

// TODO: Consider allowing config options:
// * preserve_png
// * jpg_quality=80
// * jpg_dct_scale=true
// * jpg_fix_rotation=true

pub fn _scale_and_orient(input: &[u8], max_w: u16, max_h: u16) -> Result<Vec<u8>, MultiErr> {
    let in_fmt = image::guess_format(&input)?;

    // try to use dct_scaling if possible
    if in_fmt == image::ImageFormat::Jpeg {
        match _jpeg_dct_scale(&input, max_h, max_h) {
            Ok(fast_res) => return Ok(fast_res),
            Err(_err) => {
                // log(format!("Error With DCT scale: {}", err).as_str());
            }
        }
    }

    let img = image::load_from_memory(&input)?;

    let (max_w, max_h) = (max_w as u32, max_h as u32);
    let (orig_w, orig_h) = (img.width(), img.height());

    // Use a rough scaling algorithm scaling unless original is close to the desired size
    let filter = if orig_w >= 3 * max_w && orig_h >= 3 * max_h {
        image::imageops::Nearest
    } else {
        image::imageops::Triangle
    };

    // Make sure the min dimension is at least 1px
    let thumb = if orig_h * max_w < orig_w {
        let new_w = (orig_w as f64 / orig_h as f64).round() as u32;
        img.resize_exact(new_w, 1, filter)
    } else if orig_w * max_h < orig_h {
        let new_h = (orig_h as f64 / orig_w as f64).round() as u32;
        img.resize_exact(1, new_h, filter)
    } else {
        img.resize(max_w, max_h, filter)
    };

    let thumb = match get_orientation(&input) {
        // Reference: https://www.daveperrett.com/articles/2012/07/28/exif-orientation-handling-is-a-ghetto/
        // Reference: http://sylvana.net/jpegcrop/exif_orientation.html
        Ok(2) => thumb.fliph(),
        Ok(3) => thumb.rotate180(),
        Ok(4) => thumb.flipv(),
        Ok(5) => thumb.rotate90().fliph(),
        Ok(6) => thumb.rotate90(),
        Ok(7) => thumb.rotate90().flipv(),
        Ok(8) => thumb.rotate270(),
        _ => thumb,
    };

    let output_fmt = match in_fmt {
        image::ImageFormat::Png => image::ImageOutputFormat::Png,
        _ => image::ImageOutputFormat::Jpeg(80),
    };

    let mut out: Vec<u8> = Vec::new();
    thumb.write_to(&mut out, output_fmt)?;
    Ok(out)
}

fn _jpeg_dct_scale(input: &[u8], max_w: u16, max_h: u16) -> Result<Vec<u8>, MultiErr> {
    let orientation = get_orientation(&input).unwrap_or(0);

    let mut decoder = jpeg_decoder::Decoder::new(io::Cursor::new(&input));
    decoder.scale(max_w, max_h)?;

    let metadata = decoder.info().unwrap();
    match metadata.pixel_format {
        PixelFormat::RGB24 => (),
        // We can add support for these CMYK, L8 in the future if needed.
        pixel_format => {
            return Err(NailError {
                message: format!("Unsupported format: {:?}", pixel_format),
            }
            .into())
        }
    };

    let img_buf = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
        metadata.width as u32,
        metadata.height as u32,
        decoder.decode()?,
    )
    .unwrap();

    let img_buf = match orientation {
        2 => imageops::flip_horizontal(&img_buf),
        3 => imageops::rotate180(&img_buf),
        4 => imageops::flip_vertical(&img_buf),
        5 => imageops::flip_horizontal(&imageops::rotate90(&img_buf)),
        6 => imageops::rotate90(&img_buf),
        7 => imageops::flip_vertical(&imageops::rotate90(&img_buf)),
        8 => imageops::rotate270(&img_buf),
        _ => img_buf,
    };

    // Now resize as needed
    let (max_w, max_h) = (max_w as u32, max_h as u32);
    let (width, height) = img_buf.dimensions();

    let img_buf = if height * max_w < width {
        let new_w = (width as f64 / height as f64).round() as u32;
        imageops::resize(&img_buf, new_w, 1, imageops::Triangle)
    } else if width * max_w < height {
        let new_h = (height as f64 / width as f64).round() as u32;
        imageops::resize(&img_buf, 1, new_h, imageops::Triangle)
    } else {
        let ratio = (width as f64 / max_w as f64).max(height as f64 / max_h as f64);
        let width = (width as f64 / ratio).round() as u32;
        let height = (height as f64 / ratio).round() as u32;
        imageops::resize(&img_buf, width, height, imageops::Triangle)
    };

    let out: Vec<u8> = Vec::new();
    let mut curs = io::Cursor::new(out);

    let (width, height) = img_buf.dimensions();

    let enc = JPEGEncoder::new_with_quality(&mut curs, 80);

    enc.write_image(&img_buf.as_ref(), width, height, ColorType::Rgb8)?;

    Ok(curs.into_inner())
}
