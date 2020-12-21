#[macro_use]
extern crate serde_derive;
use image::codecs::jpeg::JpegDecoder;
use image::imageops::FilterType;
use image::{
    ColorType, DynamicImage, GenericImageView, ImageDecoder, ImageFormat,
    ImageOutputFormat, Rgba, RgbaImage,
};
use std::convert::TryFrom;
use std::io::Cursor;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

use gif::{DisposalMethod};
use num_rational::Ratio;

mod errors;
use errors::MultiErr;

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
 * Resize an animated gif to fit.
 */
export function resize_gif(input: ArrayBufferLike, target_w: number, target_h: number): ArrayBufferLike;

// Expose type exports for deferred loading
interface ExposedFunctions {
  convert: typeof convert;
  image_info: typeof image_info;
  resize_gif: typeof resize_gif;
}
export type {ExposedFunctions};
"#;

#[derive(Serialize)]
pub struct ImageInfo {
    format: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
pub struct ResizeRequest {
    #[serde(with = "serde_bytes")]
    input: Vec<u8>,
    resize_op: ResizeType,

    target_w: u16,
    target_h: u16,
    down_only: bool,

    jpeg_scaling: bool,
    #[serde(with = "FilterOption")]
    scale_filter: FilterType,

    output_format: OutputFormat,
    jpeg_quality: u8,
}

#[derive(Deserialize, Serialize)]
pub enum OutputFormat {
    JPEG,
    PNG,
    Auto,
}

#[derive(Deserialize)]
#[serde(remote = "FilterType")]
pub enum FilterOption {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

#[derive(Deserialize)]
pub enum ResizeType {
    Fit,
    Cover,
    Crop,
}

impl ResizeType {
    fn cover(&self) -> bool {
        !matches!(*self, ResizeType::Fit)
    }
}

#[derive(Serialize)]
pub struct ResizeResult {
    #[serde(with = "serde_bytes")]
    output: Vec<u8>,
    format: String,
    w: u16,
    h: u16,
}

#[wasm_bindgen(skip_typescript)]
pub fn convert(val: JsValue) -> Result<JsValue, JsValue> {
    let parsed: ResizeRequest = serde_wasm_bindgen::from_value(val)?;

    match _convert(parsed) {
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
            })
        }
        Some(fmt) => fmt,
    };

    let mut format: String = format!("{:?}", fmt);
    format.make_ascii_lowercase();
    let (width, height) = reader.into_dimensions()?;

    Ok(ImageInfo {
        format,
        width,
        height,
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
    let h_ratio = target_h as f64 / orig_h as f64;
    let w_ratio = target_w as f64 / orig_w as f64;

    // default (shrink to fit) mode prefers to scale by the smaller ratio,
    // whereas cover mode scales by the larger ratio
    let ratio = if cover {
        h_ratio.max(w_ratio)
    } else {
        h_ratio.min(w_ratio)
    };

    if down_only && ratio > 1.0 {
        return (orig_w, orig_h);
    }

    // keep at least one pixel
    let scaled_w = (orig_w as f64 * ratio).round().max(1.0) as u32;
    let scaled_h = (orig_h as f64 * ratio).round().max(1.0) as u32;

    (scaled_w, scaled_h)
}

#[wasm_bindgen(skip_typescript)]
pub fn resize_gif(input: &[u8], target_w: u16, target_h: u16) -> Result<JsValue, JsValue> {
    match resize_gif_animation_internal(input, target_w, target_h) {
        Ok(result) => Ok(JsValue::from_serde(&result).unwrap()),
        Err(err) => Err(err.into()),
    }
}

/// WIP animated gif resizing
fn resize_gif_animation_internal(
    input: &[u8],
    target_w: u16,
    target_h: u16,
) -> Result<Vec<u8>, MultiErr> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);

    let curs = Cursor::new(&input);
    let mut decoder = options.read_info(curs).unwrap();

    let ratio_w = Ratio::from(target_w) / decoder.width();
    let ratio_h = Ratio::from(target_h) / decoder.height();
    let ratio = ratio_w.min(ratio_h);

    let scale = |x: u16| (ratio * x).round();

    let mut accum: image::RgbaImage = image::ImageBuffer::from_pixel(
        u32::from(decoder.width()),
        u32::from(decoder.height()),
        Rgba::from([0, 0, 0, 0]),
    );

    let mut restore = accum.clone();

    let scaled_w = scale(decoder.width());
    let scaled_h = scale(decoder.height());

    let mut frame_idx = 0;
    let mut out = vec![];
    {
        let mut encoder =
            gif::Encoder::new(&mut out, scaled_w.to_integer(), scaled_h.to_integer(), &[])?;
        encoder.set_repeat(gif::Repeat::Infinite)?;

        while let Some(frame) = decoder.read_next_frame().unwrap_or(None) {
            let raw = frame.buffer.to_vec();
            let imagebuf =
                RgbaImage::from_raw(frame.width as u32, frame.height as u32, raw).unwrap();

            // This seems to have less ringing when downscaling
            let filter = FilterType::CatmullRom;

            // TODO: consider just using gif disposal crate
            if frame.dispose == DisposalMethod::Background {
                for x in 0..accum.width() {
                    for y in 0..accum.height() {
                        *accum.get_pixel_mut(x as u32, y as u32) = Rgba([0, 0, 0, 0]);
                    }
                }
            }

            // Previous because we'll restore. Keep because we'll set up transparency...
            if frame.dispose == DisposalMethod::Previous || frame.dispose == DisposalMethod::Keep {
                for (x, y, pix) in restore.enumerate_pixels_mut() {
                    *pix = *accum.get_pixel(x, y);
                }
            }
            match frame.dispose {
                gif::DisposalMethod::Keep | DisposalMethod::Background => {
                    for (x, y, pixel) in imagebuf.enumerate_pixels() {
                        let ap = accum.get_pixel_mut(frame.left as u32 + x, frame.top as u32 + y);
                        if pixel[3] != 0x00 {
                            *ap = *pixel;
                        }
                    }
                }
                _ => {
                    for (x, y, pixel) in imagebuf.enumerate_pixels() {
                        let ap = accum.get_pixel_mut(frame.left as u32 + x, frame.top as u32 + y);
                        *ap = *pixel;
                    }
                }
            };

            let scale_top = (ratio * frame.top).to_integer() as u32;
            let scale_left = (ratio * frame.left).to_integer() as u32;
            let scale_bottom = (ratio * (frame.top + frame.height)).ceil().to_integer() as u32;
            let scale_right = (ratio * (frame.left + frame.width)).ceil().to_integer() as u32;

            // TODO: We use the Nearest filter because it doesn't produce artifacts at the edge
            //       of transparent regions.  We may want to revisit and either implement a more
            //       complex filter or switch based on the input.
            let img = image::imageops::resize(
                &accum,
                (ratio * accum.width() as u16).to_integer() as u32,
                (ratio * accum.height() as u16).to_integer() as u32,
                filter,
            );

            let scale_w = scale_right - scale_left;
            let scale_h = scale_bottom - scale_top;
            let img = image::imageops::crop_imm(&img, scale_left, scale_top, scale_w, scale_h);
            let mut img: RgbaImage = img.to_image();

            if None != frame.transparent && frame.dispose == DisposalMethod::Keep {
                // Size optimization: Encode un-changed pixels with transparency
                // FIXME: This can propagate color quantization errors which can be significant
                //        when lower-quality quantization settings are used. We should instead
                //        maintain a screen populated with the unpacked output frames to account
                //        for this.
                let prev: RgbaImage = image::imageops::resize(
                    &restore,
                    (ratio * restore.width() as u16).to_integer() as u32,
                    (ratio * restore.height() as u16).to_integer() as u32,
                    filter,
                );
                let prev =
                    image::imageops::crop_imm(&prev, scale_left, scale_top, scale_w, scale_h);
                let prev: RgbaImage = prev.to_image();
                for (x, y, current) in img.enumerate_pixels_mut() {
                    let prev = prev.get_pixel(x, y);
                    if *prev == *current {
                        *current = Rgba([0, 0, 0, 0]);
                    }
                }
            }

            let mut out_frame =
                gif::Frame::from_rgba_speed(img.width() as u16, img.height() as u16, &mut img, 1);

            out_frame.delay = frame.delay;
            out_frame.dispose = frame.dispose;
            // out_frame.transparent
            out_frame.top = scale_top as u16;
            out_frame.left = scale_left as u16;
            encoder.write_frame(&out_frame)?;
            frame_idx += 1;

            if frame.dispose == DisposalMethod::Previous {
                for (x, y, pix) in restore.enumerate_pixels() {
                    let target = accum.get_pixel_mut(x, y);
                    *target = *pix;
                }
            }
        }

        if frame_idx == 0 {
            // TODO: Return error if present...
            // TODO: Should we render the background color?
            panic!("no frames");
        }
    }

    Ok(out)
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
        assert_eq!(scale_dimensions(1, 512, 32, 32, false, false), (1, 32));
        assert_eq!(scale_dimensions(512, 1, 32, 32, false, false), (32, 1));
    }
}
