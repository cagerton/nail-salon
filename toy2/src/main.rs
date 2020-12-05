use clap::{App, Arg, SubCommand};
use image::codecs::gif::{GifDecoder, GifEncoder};
use image::codecs::png::{ApngDecoder, PngDecoder, PngEncoder};
use image::imageops::FilterType;
use image::{
    AnimationDecoder, DynamicImage, GenericImage, GenericImageView, ImageBuffer, ImageDecoder,
    ImageError, ImageFormat, Rgb, Rgba,
};

use num_rational::Ratio;
use png::ColorType::RGBA;
use std::any::Any;
use std::borrow::Borrow;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::Cursor;
use std::ops::Mul;
use std::option::Option::Some;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let matches = App::new("Animation Test")
        .version("0.1.0")
        .arg(
            Arg::with_name("file")
                .required(true)
                .short("f")
                .long("file")
                .takes_value(true)
                .help("input"),
        )
        .arg(Arg::with_name("info"))
        .arg(Arg::with_name("split"))
        .get_matches();

    let filename = matches.value_of("file").expect("no input");
    let path = Path::new(filename);
    let stem = path.file_stem().expect("path stem fail").to_string_lossy();

    println!("The file passed is: {}", filename);

    let file = File::open(path)?;
    let mut buf_reader = BufReader::new(file);
    let mut buf: Vec<u8> = vec![];

    buf_reader.read_to_end(&mut buf);

    let fmt = image::guess_format(&buf).expect("image format unknown");
    println!("Got format: {:?}", fmt);

    let req = ResizeRequest {
        input: buf,
        // resize_op: ResizeType,
        target_w: 64,
        target_h: 64,
        // down_only: bool,
        // scale_filter: FilterType,
        // output_format: OutputFormat,
    };

    if let Some(_) = matches.value_of("info") {
        display_gif_details(path).expect("details failed");
        return Ok(());
    };

    match fmt {
        ImageFormat::Png => {
            if let Err(err) = png_info(req) {
                eprintln!("Error: {}", err);
            }
        }
        ImageFormat::Gif => match naive_resize(req) {
            Ok(res) => {
                let dest = format!("./thumbs/{}_thumb.gif", stem);
                let mut f = File::create(dest)?;
                f.write_all(&res)?;
            }
            Err(err) => eprintln!("Error: {:?}", err),
        },
        fmt => {
            println!("Unsupported fmt: {:?}", fmt);
        }
    };
    Ok(())
}

pub enum OutputFormat {
    // JPEG,
    PNG,
    GIF,
    Auto,
}

pub enum FilterOption {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

#[derive(PartialEq)]
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

pub struct ResizeRequest {
    input: Vec<u8>,
    // resize_op: ResizeType,
    target_w: u16,
    target_h: u16,
    // down_only: bool,
    // scale_filter: FilterType,
    // output_format: OutputFormat,
}

#[derive(Debug)]
pub enum SomeError {
    IoError(std::io::Error),
    ImageError(ImageError),
    DecodingError(gif::DecodingError),
    EncodingError(gif::EncodingError),
}

impl From<gif::EncodingError> for SomeError {
    fn from(err: gif::EncodingError) -> SomeError {
        SomeError::EncodingError(err)
    }
}

impl From<std::io::Error> for SomeError {
    fn from(err: std::io::Error) -> SomeError {
        SomeError::IoError(err)
    }
}

impl From<image::ImageError> for SomeError {
    fn from(err: image::ImageError) -> SomeError {
        SomeError::ImageError(err)
    }
}

/// cheap hax
fn palette_lookup(palette: &[u8], index: usize) -> Option<Rgba<u8>> {
    if let Some(chunk) = palette.chunks_exact(3).nth(index) {
        assert_eq!(chunk.len(), 3);
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        Some(image::Rgba::from([r, g, b, 0xff]))
    } else {
        None
    }
}

fn display_gif_details(path: &Path) -> Result<(), SomeError> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);

    let stem = path.file_stem().expect("path stem fail").to_string_lossy();

    let input = std::fs::read(path)?;
    let curs = Cursor::new(input);

    let mut decoder = options.read_info(curs).unwrap();

    println!(
        "\n\nGIF: {:?}   dims: {:?}",
        path,
        (decoder.width(), decoder.height())
    );

    let global_palette = decoder.global_palette().unwrap_or(&[]);

    let bg_pixel = if let Some(bg_idx) = decoder.bg_color() {
        palette_lookup(&global_palette, bg_idx)
    } else {
        None
    };

    let default_pixel = bg_pixel.unwrap_or(Rgba::from([0, 0, 0, 0xff]));
    println!("default pixel: {:?}", default_pixel);

    let mut idx = 0;
    {
        // fixme
        while let Some(frame) = decoder.read_next_frame().unwrap_or(None) {
            // Process every frame
            println!(
                "\tFrame: {}, pos: {:?}, size: {:?}, delay: {:?}, disp: {:?}, transp: {:?}",
                idx,
                (frame.left, frame.top),
                (frame.width, frame.height),
                frame.delay,
                frame.dispose,
                frame.transparent,
            );

            let raw = frame.buffer.to_vec();

            let mut imagebuf =
                image::RgbaImage::from_raw(u32::from(frame.width), u32::from(frame.height), raw)
                    .unwrap();

            let img = DynamicImage::ImageRgba8(imagebuf);
            img.save_with_format(
                format!("./frames/{}_{:04}.png", stem, idx),
                image::ImageFormat::Png,
            )?;

            idx = idx + 1;
            if idx > 10 {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// this is garbage - but hey, it seems to work
fn naive_resize(request: ResizeRequest) -> Result<Vec<u8>, SomeError> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);

    let curs = Cursor::new(request.input);
    let mut decoder = options.read_info(curs).unwrap();

    println!("dims (w/h): {:?} x {:?}", decoder.width(), decoder.height());
    println!("bgcolor: {:?}", decoder.bg_color());

    let ratio_w = Ratio::from(request.target_w) / decoder.width();
    let ratio_h = Ratio::from(request.target_h) / decoder.height();
    let ratio = ratio_w.min(ratio_h);

    let scale = |x: u16| (ratio * x).round();
    let global_palette = decoder.global_palette().unwrap_or(&[]);
    println!("global_palette: {:?}", global_palette);

    let bg_pixel = if let Some(bg_idx) = decoder.bg_color() {
        palette_lookup(&global_palette, bg_idx)
    } else {
        None
    };
    let default_pixel = bg_pixel.unwrap_or(Rgba::from([0, 0, 0, 0xff]));
    println!("default pixel: {:?}", default_pixel);

    let mut accum: image::RgbaImage = image::ImageBuffer::from_pixel(
        u32::from(decoder.width()),
        u32::from(decoder.height()),
        default_pixel,
    );

    let scaled_w = scale(decoder.width());
    let scaled_h = scale(decoder.height());
    println!("output will be {}x{}", scaled_w, scaled_h);

    let mut frame_idx = 0;
    let mut out = vec![];
    {
        let mut encoder = gif::Encoder::new(
            &mut out,
            scaled_w.to_integer(),
            scaled_h.to_integer(),
            global_palette,
        )?;

        encoder.set_repeat(gif::Repeat::Infinite)?;

        // we'll stop if we get an error here
        while let Some(mut frame) = decoder.read_next_frame().unwrap_or(None) {
            // Process every frame
            // println!("frame: {:?}", frame);
            let raw = frame.buffer.to_vec();

            let mut imagebuf =
                image::RgbaImage::from_raw(u32::from(frame.width), u32::from(frame.height), raw)
                    .unwrap();

            /// Frame disposal applies to _this_ frame, not the previous one.
            /// Saving this for the lulz
            println!("{:?}", frame.dispose);
            match frame.dispose {
                gif::DisposalMethod::Keep => {
                    for (x, y, pixel) in imagebuf.enumerate_pixels() {
                        let ap = accum.get_pixel_mut(frame.left as u32 + x, frame.top as u32 + y);
                        if pixel[3] != 0x00 {
                            *ap = *pixel;
                        }
                    }
                }

                gif::DisposalMethod::Background => {
                    for pixel in accum.pixels_mut() {
                        *pixel = Rgba::from([0, 0, 0, 0]);
                    }
                    for (x, y, pixel) in imagebuf.enumerate_pixels() {
                        let ap = accum.get_pixel_mut(frame.left as u32 + x, frame.top as u32 + y);
                        if pixel[3] == 0xff {
                            *ap = *pixel;
                        } else {
                            // *ap = default_pixel;
                        }
                    }
                }

                // We'll ignore the "previous" option for now
                _ => {
                    std::mem::swap(&mut accum, &mut imagebuf);
                }
            };

            let img = DynamicImage::ImageRgba8(accum.clone());
            img.save_with_format(
                format!("./fullframes/frame_{:04}.png", frame_idx),
                image::ImageFormat::Png,
            )?;

            let mut img = img.resize_exact(
                (ratio * img.width() as u16).to_integer() as u32,
                (ratio * img.height() as u16).to_integer() as u32,
                image::imageops::Lanczos3,
            );
            img.save_with_format(
                format!("./resizedframes/frame_{:04}.png", frame_idx),
                image::ImageFormat::Png,
            )?;

            let scale_top = (ratio * frame.top).to_integer() as u32;
            let scale_left = (ratio * frame.left).to_integer() as u32;
            let scale_bottom = (ratio * (frame.top + frame.height)).ceil().to_integer() as u32;
            let scale_right = (ratio * (frame.left + frame.width)).ceil().to_integer() as u32;

            // let mut sub = img.sub_image(
            //     scale_left,
            //     scale_top,
            //     scale_right - scale_left,
            //     scale_bottom - scale_top
            // );

            let scale_w = scale_right - scale_left;
            let scale_h = scale_bottom - scale_top;

            /// lol facepalm
            let buf = img.clone();
            let buf = buf.crop_imm(scale_left, scale_top, scale_w, scale_h);
            // img.crop(scale_left, scale_top, scale_w, scale_h);
            let mut imagebuf = buf.into_rgba8();

            let mut out_frame = gif::Frame::from_rgba(
                imagebuf.width() as u16,
                imagebuf.height() as u16,
                &mut imagebuf,
            );

            out_frame.delay = frame.delay;
            out_frame.dispose = frame.dispose;

            out_frame.top = scale_top as u16;
            out_frame.left = scale_left as u16;
            encoder.write_frame(&out_frame)?;
            frame_idx += 1;
        }

        if frame_idx == 0 {
            // fixme add err?
            panic!("no frames");
        }
    }

    Ok(out)
}

fn read_write_fullframe(request: ResizeRequest) -> Result<Vec<u8>, SomeError> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);

    let curs = Cursor::new(request.input);
    let mut decoder = options.read_info(curs).unwrap();
    let global_palette = decoder.global_palette().unwrap_or(&[]);

    println!("dims (w/h): {:?} x {:?}", decoder.width(), decoder.height());
    println!("bgcolor: {:?}", decoder.bg_color());

    println!("global_palette: {:?}", global_palette);

    let mut out = vec![];
    {
        let mut encoder =
            gif::Encoder::new(&mut out, decoder.width(), decoder.height(), global_palette)?;
        encoder.set_repeat(gif::Repeat::Infinite)?;

        // let mut frame = ImageBuffer::new(
        //     u32::from(decoder.width()),
        //     u32::from(decoder.height()));

        // todo: bgframe?
        while let Some(mut frame) = decoder.read_next_frame().unwrap() {
            // Process every frame
            println!("frame: {:?}", frame);
            let raw = frame.buffer.to_vec();

            let mut imagebuf =
                image::RgbaImage::from_raw(u32::from(frame.width), u32::from(frame.height), raw)
                    .unwrap();

            // let mut colormapped = gif::Frame::from_rgba(frame.width, frame.height, &mut imagebuf);
            let mut colormapped =
                gif::Frame::from_rgba_speed(frame.width, frame.height, &mut imagebuf, 30);

            colormapped.delay = frame.delay;
            colormapped.dispose = frame.dispose;
            colormapped.top = frame.top;
            colormapped.left = frame.left;
            // let mut new_frame = frame.clone();
            // new_frame.buffer = colormapped.buffer;

            encoder.write_frame(&colormapped)?;
        }
    }
    Ok(out)
}

fn gif_info(request: ResizeRequest) -> Result<Vec<u8>, ImageError> {
    println!("raw_bytes: {}", request.input.len());
    let mut decoder = GifDecoder::new(Cursor::new(request.input))?;
    // let mut decoder = gif::Decoder::new(Cursor::new(request.input))?;
    // decoder.
    // let f = decoder.read_next_frame();
    println!("dimensions: {:?}", decoder.dimensions());
    println!("type_id: {:?}", decoder.type_id());
    println!("orig_color_type: {:?}", decoder.original_color_type());
    println!("color_type: {:?}", decoder.color_type());
    println!("totalbytes: {:?}", decoder.total_bytes());
    println!("scanlinebytes: {:?}", decoder.scanline_bytes());

    // let frames = decoder.into_frames();

    // let frames.map(|f| {})
    // let frames = frames.collect_frames().expect("error decoding gif");

    let frames = decoder.into_frames();
    let mut out: Vec<u8> = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        encoder.try_encode_frames(frames)?;
    }
    Ok(out)
}

fn png_info(request: ResizeRequest) -> Result<Vec<u8>, ImageError> {
    println!("raw_bytes: {}", request.input.len());
    let decoder = PngDecoder::new(Cursor::new(request.input))?;

    println!("dimensions: {:?}", decoder.dimensions());
    println!("type_id: {:?}", decoder.type_id());
    println!("orig_color_type: {:?}", decoder.original_color_type());
    println!("color_type: {:?}", decoder.color_type());
    println!("totalbytes: {:?}", decoder.total_bytes());
    println!("scanlinebytes: {:?}", decoder.scanline_bytes());

    println!("is_apng: {}", decoder.is_apng());
    let apng = decoder.apng();
    let frames = apng.into_frames().collect_frames()?;

    let mut i = 0;
    for f in frames {
        println!("Frame[{}]", i);
        println!("\tdelay: {:?}", f.delay());
        println!("\t(left={}, top={})", f.left(), f.top());
        i += 1;
    }
    Ok(vec![])
}
