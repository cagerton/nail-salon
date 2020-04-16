extern crate wasm_bindgen;
// extern crate wee_alloc;

use image::{jpeg, save_buffer_with_format, ImageEncoder};
use image::ColorType;
use jpeg_decoder;
use jpeg::JPEGEncoder;
use image::imageops::{flip_horizontal, rotate180, flip_vertical, rotate90, rotate270};
use image;
use wasm_bindgen::prelude::*;

use std::io::Cursor;
// Use `wee_alloc` as the global allocator.
// #[global_allocator]
// static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;


fn get_orientation(input: &[u8]) -> Result<u32, exif::Error> {
    // let reader = BufReader::from(&input);
    let exifreader = exif::Reader::new();
    let mut cursor = Cursor::new(input);
    let data = exifreader.read_from_container(&mut cursor)?;

    let o_field = data.get_field(exif::Tag::Orientation, exif::In::PRIMARY);
    return match o_field {
        Some(res) => Ok(res.value.get_uint(0).unwrap_or(0)),
        _ => Ok(0)
    }
}
#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_u32(a: u32);
}

    #[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}


#[wasm_bindgen]
pub fn thumb_fix(input: &[u8], max_dim: u32) -> Option<Vec<u8>> {
    let ouput_fmt = image::ImageFormat::Jpeg;

    let orientation = get_orientation(input);
    let img = image::load_from_memory(&input).unwrap();
    let thumb = img.thumbnail(max_dim, max_dim);

    let transformed = match orientation {
        // Reference: https://www.daveperrett.com/articles/2012/07/28/exif-orientation-handling-is-a-ghetto/
        // Reference: http://sylvana.net/jpegcrop/exif_orientation.html
        Ok(1) => thumb,
        Ok(2) => thumb.fliph(),
        Ok(3) => thumb.rotate180(),
        Ok(4) => thumb.flipv(),
        Ok(5) => thumb.rotate90().fliph(),
        Ok(6) => thumb.rotate90(),
        Ok(7) => thumb.rotate90().flipv(),
        Ok(8) => thumb.rotate270(),
        _ => thumb
    };

    log_u32(orientation.unwrap_or(0));
    let mut out: Vec<u8> = Vec::new();
    transformed.write_to(&mut out, ouput_fmt).expect("Failed to write output");
    return Some(out);
}

#[wasm_bindgen]
pub fn fast_thumb_fix(input: &[u8], max_dim: u16) -> Option<Vec<u8>> {
    let ouput_fmt = image::ImageFormat::Jpeg;
    let orientation = get_orientation(input).unwrap_or(0);

    let mut dec = jpeg_decoder::Decoder::new(Cursor::new(input));
    dec.scale(max_dim, max_dim).unwrap();
    dec.read_info().unwrap();
    let info = dec.info().unwrap();
    let raw = dec.decode().unwrap();

    let img_buf = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(info.width as u32, info.height as u32, raw).unwrap();


    let img_buf = match orientation {
        2 => flip_horizontal(&img_buf),
        3 => rotate180(&img_buf),
        4 => flip_vertical(&img_buf),
        // FIXME: Image artifacts on rotations 2,3,4. Falling back to re-parsing
        // 5 => flip_horizontal(&rotate90(&img_buf)),
        // 6 => rotate90(&img_buf),
        // 7 => flip_vertical(&rotate90(&img_buf)),
        // 8 => rotate270(&img_buf),
        _ => img_buf
    };

    let out: Vec<u8> = Vec::new();
    let mut curs = Cursor::new(out);

    // let enc = JPEGEncoder::new_with_quality(&mut curs, 90);
    let enc = JPEGEncoder::new(&mut curs);
    enc.write_image(&img_buf.into_vec(),
                    info.width as u32,
                    info.height as u32,
                    ColorType::Rgb8).expect("encoded");

    // FIXME: Figure out why rotation is broken..
    if orientation >= 5 {
        let thumb = image::load_from_memory(&curs.into_inner()).unwrap();
        let thumb = match orientation {
            // Reference: https://www.daveperrett.com/articles/2012/07/28/exif-orientation-handling-is-a-ghetto/
            // Reference: http://sylvana.net/jpegcrop/exif_orientation.html
            // 2 => thumb.fliph(),
            // 3 => thumb.rotate180(),
            // 4 => thumb.flipv(),
            5 => thumb.rotate90().fliph(),
            6 => thumb.rotate90(),
            7 => thumb.rotate90().flipv(),
            8 => thumb.rotate270(),
            _ => thumb
        };
        let mut out: Vec<u8> = Vec::new();
        thumb.write_to(&mut out, ouput_fmt).expect("Failed to write output");
        return Some(out);
    }

    return Some(curs.into_inner());
}
