#[macro_use]
extern crate serde_derive;
use image::codecs::jpeg::JpegDecoder;
use image::{
    ColorType, DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageOutputFormat,
};
use num_rational::Ratio;
use std::convert::TryFrom;
use std::io::Cursor;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

mod errors;
mod giflib;
mod utils;

use errors::MultiErr;
use utils::*;

#[wasm_bindgen]
pub fn version() -> String {
    utils::VERSION.into()
}

// TODO: Generate TS definitions
#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT: &'static str = r#"
import {ResizeRequest, ImageInfo, ResizeResult} from '../lib/types';

/**
 * Resize an image
 */
export function convert(req: ResizeRequest): ResizeResult;

/**
 * Return image format and dimensions
 */
export function image_info(input: Uint8Array): ImageInfo;

/**
 * NailSalon version
 */
export function version(): String;

// Expose type exports for deferred loading
interface ExposedFunctions {
  convert: typeof convert;
  image_info: typeof image_info;
  version: typeof version;
}
export type {ExposedFunctions};
"#;

#[wasm_bindgen(skip_typescript)]
pub fn convert(val: JsValue) -> Result<JsValue, JsValue> {
    let parsed: ResizeRequest = serde_wasm_bindgen::from_value(val)?;

    match _convert(parsed) {
        Ok(result) => Ok(serde_wasm_bindgen::to_value(&result)?),
        Err(err) => Err(err.into()),
    }
}

#[wasm_bindgen(skip_typescript)]
pub fn resize_animation(val: JsValue) -> Result<JsValue, JsValue> {
    let parsed: ResizeRequest = serde_wasm_bindgen::from_value(val)?;

    match giflib::resize_animation(parsed) {
        Ok(result) => Ok(serde_wasm_bindgen::to_value(&result)?),
        Err(err) => Err(err.into()),
    }
}

fn get_orientation(input: &[u8]) -> Result<u32, exif::Error> {
    let data = exif::Reader::new().read_from_container(&mut Cursor::new(&input))?;
    match data.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        Some(res) => Ok(res.value.get_uint(0).unwrap_or(0)),
        _ => Ok(0),
    }
}

fn _convert(request: ResizeRequest) -> Result<ResizeResult, MultiErr> {
    let in_fmt = image::guess_format(&request.input)?;

    if request.support_animation
        && in_fmt == ImageFormat::Gif
        && is_animated_gif(&request.input).unwrap_or(false)
    {
        return giflib::resize_animation(request);
    }

    let orientation = if in_fmt == ImageFormat::Jpeg {
        get_orientation(&request.input).unwrap_or(0)
    } else {
        0
    };

    let is_transposed = orientation > 4;
    let cover = request.resize_op.cover();

    let (downscale_w, downscale_h) = if is_transposed {
        (request.target_h, request.target_w)
    } else {
        (request.target_w, request.target_h)
    };

    let img = if in_fmt == image::ImageFormat::Jpeg && request.jpeg_scaling {
        let mut decoder = JpegDecoder::new(Cursor::new(&request.input))?;

        // Setup DCT scaling. Use a larger size than needed since we still need to resize.
        let (w, h) = decoder.dimensions();
        let (req_w, req_h) = scale_dimensions(
            w,
            h,
            downscale_w as u32 * 2,
            downscale_h as u32 * 2,
            cover,
            true,
        );
        decoder.scale(u16::try_from(req_w)?, u16::try_from(req_h).unwrap())?;

        DynamicImage::from_decoder(decoder)?
    } else {
        image::load_from_memory(&request.input)?
    };

    let (orig_w, orig_h) = (img.width(), img.height());

    let thumb = match request.resize_op {
        ResizeType::Crop => {
            img.resize_to_fill(downscale_w as u32, downscale_h as u32, request.scale_filter)
        }
        _ => {
            let (resized_w, resized_h) = scale_dimensions(
                orig_w,
                orig_h,
                downscale_w as u32,
                downscale_h as u32,
                cover,
                request.down_only,
            );
            img.resize_exact(resized_w as u32, resized_h as u32, request.scale_filter)
            // img.thumbnail_exact(resized_w, resized_h) //, request.scale_filter)
        }
    };

    // Correct orientation
    let thumb = match orientation {
        // Reference: https://www.daveperrett.com/articles/2012/07/28/exif-orientation-handling-is-a-ghetto/
        // Reference: http://sylvana.net/jpegcrop/exif_orientation.html
        2 => thumb.fliph(),
        3 => thumb.rotate180(),
        4 => thumb.flipv(),
        5 => thumb.rotate90().fliph(),
        6 => thumb.rotate90(),
        7 => thumb.rotate90().flipv(),
        8 => thumb.rotate270(),
        _ => thumb,
    };

    let output_fmt = match request.output_format {
        OutputFormat::Auto => {
            if in_fmt == ImageFormat::Png {
                in_fmt
            } else {
                ImageFormat::Jpeg
            }
        }
        OutputFormat::JPEG => ImageFormat::Jpeg,
        OutputFormat::PNG => ImageFormat::Png,
    };

    let (fmt_name, output_fmt) = match output_fmt {
        image::ImageFormat::Png => ("PNG", ImageOutputFormat::Png),
        _ => ("JPEG", ImageOutputFormat::Jpeg(request.jpeg_quality)),
    };

    // Normalize pixel format prior to encoding
    // 16 bit depth is unsupported for JPEG, buggy with the PNG encoder
    // BGR is unsupported by the PNG encoder.
    let thumb = match thumb.color() {
        ColorType::L16 => DynamicImage::ImageLuma8(thumb.into_luma8()),
        ColorType::La16 => DynamicImage::ImageLumaA8(thumb.into_luma_alpha8()),
        ColorType::Rgb16 | ColorType::Bgr8 => DynamicImage::ImageRgb8(thumb.into_rgb8()),
        ColorType::Rgba16 | ColorType::Bgra8 => DynamicImage::ImageRgba8(thumb.into_rgba8()),
        _ => thumb,
    };

    let mut out: Vec<u8> = Vec::new();
    thumb.write_to(&mut out, output_fmt)?;

    let (w, h) = thumb.dimensions();
    Ok(ResizeResult {
        format: fmt_name.into(),
        output: out,
        version: VERSION.into(),
        w: u16::try_from(w).unwrap(),
        h: u16::try_from(h).unwrap(),
    })
}

#[wasm_bindgen(skip_typescript)]
pub fn image_info(input: &[u8]) -> Result<JsValue, JsValue> {
    match _image_info(input) {
        Ok(result) => Ok(JsValue::from_serde(&result).unwrap()),
        Err(err) => Err(err.into()),
    }
}

pub fn _image_info(input: &[u8]) -> Result<ImageInfo, MultiErr> {
    let reader = image::io::Reader::new(Cursor::new(&input))
        .with_guessed_format()
        .unwrap();

    let fmt = match reader.format() {
        None => {
            return Ok(ImageInfo {
                format: "unknown".to_string(),
                width: 0,
                height: 0,
                animated: false,
            })
        }
        Some(fmt) => fmt,
    };

    let animated = if ImageFormat::Gif == fmt {
        is_animated_gif(&input)?
    } else {
        // TODO: add support for other formats
        false
    };

    let mut format: String = format!("{:?}", fmt);
    format.make_ascii_lowercase();
    let (width, height) = reader.into_dimensions()?;

    Ok(ImageInfo {
        format,
        width,
        height,
        animated,
    })
}

/// Treats any gif with multiple frames as an animated gif.
/// TODO: confirm that multiple frames aren't commonly used for other purposes.
fn is_animated_gif(input: &[u8]) -> Result<bool, gif::DecodingError> {
    let mut d = gif::Decoder::new(Cursor::new(&input))?;
    d.next_frame_info()?;
    Ok(match d.next_frame_info()? {
        Some(_) => true,
        _ => false,
    })
}

pub fn scale_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: u32,
    target_h: u32,
    cover: bool,
    down_only: bool,
) -> (u32, u32) {
    let h_ratio = Ratio::new(target_h, orig_h);
    let w_ratio = Ratio::new(target_w, orig_w);

    // default (shrink to fit) mode prefers to scale by the smaller ratio,
    // whereas cover mode scales by the larger ratio
    let ratio = if cover {
        h_ratio.max(w_ratio)
    } else {
        h_ratio.min(w_ratio)
    };

    let ratio = if down_only {
        ratio.min(Ratio::from_integer(1))
    } else {
        ratio
    };

    // keep at least one pixel
    let scaled_w = (ratio * orig_w).round().to_integer().max(1);
    let scaled_h = (ratio * orig_h).round().to_integer().max(1);

    (scaled_w, scaled_h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

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
        assert_eq!(scale_dimensions(1, 512, 32, 32, false, false), (1, 32));
        assert_eq!(scale_dimensions(512, 1, 32, 32, false, false), (32, 1));
    }

    #[test]
    fn test_convert_and_info() {
        // Create a JPG
        let img = ImageBuffer::from_pixel(100, 100, Rgba::<u8>([0, 0, 0, 255]));
        let img = DynamicImage::ImageRgba8(img);
        let mut raw_jpg = vec![];
        img.write_to(&mut raw_jpg, ImageOutputFormat::Jpeg(80))
            .unwrap();

        // Read the JPEG
        let info = _image_info(&raw_jpg.as_slice()).unwrap();
        assert_eq!(info.format, "jpeg");
        assert_eq!(info.height, 100);
        assert_eq!(info.width, 100);
        assert_eq!(info.animated, false);

        // Convert to PNG
        let res = _convert(ResizeRequest {
            input: raw_jpg,
            resize_op: ResizeType::Fit,
            target_h: 64,
            target_w: 64,
            down_only: true,
            jpeg_quality: 80,
            jpeg_scaling: false,
            scale_filter: image::imageops::CatmullRom,
            output_format: OutputFormat::PNG,
            support_animation: false,
        })
        .unwrap();

        // Confirm that the output is as expected
        let info = _image_info(&res.output.as_slice()).unwrap();
        assert_eq!(info.format, "png");
        assert_eq!(info.height, 64);
        assert_eq!(info.width, 64);
        assert_eq!(info.animated, false);
    }
}
