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
pub fn scale_and_orient(
    input: &[u8],
    target_w: u16,
    target_h: u16,
    cover: bool,
    down_only: bool,
) -> Result<Vec<u8>, JsValue> {
    Ok(_scale_and_orient(
        &input, target_w, target_h, cover, down_only,
    )?)
}

// TODO: Consider allowing config options:
// * preserve_png
// * jpg_quality=80
// * jpg_dct_scale=true
// * jpg_fix_rotation=true

pub fn _scale_and_orient(
    input: &[u8],
    target_w: u16,
    target_h: u16,
    cover: bool,
    down_only: bool,
) -> Result<Vec<u8>, MultiErr> {
    let in_fmt = image::guess_format(&input)?;

    // try to use dct_scaling if possible
    if in_fmt == image::ImageFormat::Jpeg {
        match _jpeg_dct_scale(&input, target_h, target_w, cover, down_only) {
            Ok(fast_res) => return Ok(fast_res),
            Err(_err) => {
                // log(format!("Error With DCT scale: {}", err).as_str());
            }
        }
    }
    let img = image::load_from_memory(&input)?;

    let (orig_w, orig_h) = (img.width(), img.height());
    let (resized_w, resized_h) = scale_dimensions(
        orig_w,
        orig_h,
        target_w as u32,
        target_h as u32,
        cover,
        down_only,
    );

    // Use a rough scaling algorithm scaling unless original is close to the desired size
    // TODO: consider a cleaner formula here
    let filter = if orig_w > (3 * resized_w) {
        image::imageops::Nearest
    } else {
        image::imageops::Triangle
    };

    let thumb = img.resize_exact(resized_w, resized_h, filter);
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

fn _jpeg_dct_scale(
    input: &[u8],
    target_w: u16,
    target_h: u16,
    cover: bool,
    down_only: bool,
) -> Result<Vec<u8>, MultiErr> {
    let orientation = get_orientation(&input).unwrap_or(0);

    let mut decoder = jpeg_decoder::Decoder::new(io::Cursor::new(&input));
    decoder.scale(target_w, target_h)?;

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

    let (scaled_w, scaled_h) = img_buf.dimensions();
    let (resized_w, resized_h) = scale_dimensions(
        scaled_w,
        scaled_h,
        target_w as u32,
        target_h as u32,
        cover,
        down_only,
    );

    let img_buf = imageops::resize(&img_buf, resized_w, resized_h, imageops::Triangle);

    let out: Vec<u8> = Vec::new();
    let mut curs = io::Cursor::new(out);

    let (resized_w, resized_h) = img_buf.dimensions();

    let enc = JPEGEncoder::new_with_quality(&mut curs, 80);

    enc.write_image(&img_buf.as_ref(), resized_w, resized_h, ColorType::Rgb8)?;

    Ok(curs.into_inner())
}

pub fn scale_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: u32,
    target_h: u32,
    cover: bool,
    down_only: bool,
) -> (u32, u32) {
    let h_ratio = target_h as f64 / orig_h as f64;
    let w_ratio = target_w as f64 / orig_w as f64;

    // default (shrink to fit) mode prefers to scale by the smaller ratio,
    // whereas cover mode scales by the larger ratio
    let ratio = if cover ^ (h_ratio > w_ratio) {
        w_ratio
    } else {
        h_ratio
    };

    if down_only && ratio > 1.0 {
        return (orig_w, orig_h);
    }

    let scaled_w = (orig_w as f64 * ratio).round() as u32;
    let scaled_h = (orig_h as f64 * ratio).round() as u32;

    // keep at least one pixel
    if scaled_w == 0 {
        return ((orig_w as f64 / orig_h as f64).round() as u32, 1);
    }
    if scaled_h == 0 {
        return (1, (orig_h as f64 / orig_w as f64).round() as u32);
    }

    (scaled_w, scaled_h)
}

#[derive(Serialize)]
pub struct ImageInfo {
    format: String,
    width: u32,
    height: u32,
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
export interface ImageInfo {
    format: string,
    width: number,
    height: number
}

export function image_info(input: Uint8Array): ImageInfo;
"#;

#[wasm_bindgen(skip_typescript)]
pub fn image_info(input: &[u8]) -> Result<JsValue, JsValue> {
    Ok(JsValue::from_serde(&_image_info(&input)?).unwrap())
    // Ok(_image_info(&input)?)
}

pub fn _image_info(input: &[u8]) -> Result<ImageInfo, MultiErr> {
    let reader = image::io::Reader::new(std::io::Cursor::new(&input))
        .with_guessed_format()
        .expect("Cursor io never fails");

    let mut format: String = format!("{:?}", reader.format().unwrap());
    format.make_ascii_lowercase();
    let (width, height) = reader.into_dimensions()?;

    Ok(ImageInfo {
        format,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_dimensions() {
        // no-scale
        assert_eq!(scale_dimensions(64, 64, 64, 64, true, true), (64, 64));
        assert_eq!(scale_dimensions(1, 1, 64, 64, true, true), (1, 1));
        assert_eq!(scale_dimensions(64, 1, 64, 64, true, true), (64, 1));
        assert_eq!(scale_dimensions(1, 64, 64, 64, true, true), (1, 64));

        // fit
        assert_eq!(scale_dimensions(64, 32, 32, 32, false, true), (32, 16));
        assert_eq!(scale_dimensions(32, 64, 32, 32, false, true), (16, 32));

        // cover
        assert_eq!(scale_dimensions(64, 32, 32, 32, true, true), (64, 32));
        assert_eq!(scale_dimensions(32, 64, 32, 32, true, true), (32, 64));

        assert_eq!(scale_dimensions(64, 32, 16, 16, true, true), (32, 16));
        assert_eq!(scale_dimensions(32, 64, 16, 16, true, true), (16, 32));

        // narrow cover, down only
        assert_eq!(scale_dimensions(64, 16, 32, 32, true, true), (64, 16));

        // narrow cover, up
        assert_eq!(scale_dimensions(64, 16, 32, 32, true, false), (128, 32));

        // narrow down-only
        assert_eq!(scale_dimensions(64, 2, 32, 32, false, true), (32, 1));
        assert_eq!(scale_dimensions(2, 64, 32, 32, false, true), (1, 32));

        // narrow fit, up
        assert_eq!(scale_dimensions(8, 16, 32, 32, false, false), (16, 32));
        assert_eq!(scale_dimensions(16, 8, 32, 32, false, false), (32, 16));
    }
}
