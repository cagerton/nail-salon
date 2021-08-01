use crate::errors::MultiErr;
use crate::utils::{ResizeRequest, ResizeResult, ResizeType, VERSION};
use gif::{Decoder, DisposalMethod, Repeat};
use gif_dispose::Screen;
use image::imageops::FilterType;
use image::{GenericImageView, ImageBuffer, Rgba, RgbaImage};
use num_rational::Ratio;
use std::io::Read;

pub struct FrameIter<T: Read> {
    decoder: Decoder<T>,
    screen: Screen,
}

impl<T: Read> FrameIter<T> {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.decoder.width() as u32, self.decoder.height() as u32)
    }
}

pub struct RenderedFrame {
    dispose: DisposalMethod,
    delay: u16,
    rendered: RgbaImage,
    left_top: (u32, u32),
    size: (u32, u32),
}

/// Adjusts the alpha depth to 1bit to minimize color distortion at the
/// edge of transparent regions. There are lots of ways we could improve this,
/// but really we should probably just encode with a format that supports a
/// greater alpha depth.
fn flatten_alpha_depth(pix: &Rgba<u8>) -> Rgba<u8> {
    if pix[3] < 0x80 {
        // 0x80 is 50% opacity
        Rgba([0, 0, 0, 0])
    } else {
        // Our pixel was the blended combination of some transparent Rgba([0, 0, 0, 0])
        // pixels and some pixels that average to Rgba([r, g, b, 255]). Here we want to
        // approximate the latter.
        let factor = Ratio::new(255 - pix[3] as u32, 255);
        let adj = |x: u8| {
            let x = u32::from(x);
            (x + (factor * (255 - x)).ceil().to_integer()) as u8
        };

        Rgba([adj(pix[0]), adj(pix[1]), adj(pix[2]), 0xff])
    }
}

impl RenderedFrame {
    /// Returns an image containing the frame's portion of the screen.
    /// If the disposal method is keep, any unchanged pixels are marked as transparent.
    pub fn clip_changes(&self, base: &RgbaImage) -> RgbaImage {
        assert_eq!(self.rendered.dimensions(), base.dimensions());
        assert!(self.size.0 + self.left_top.0 <= self.rendered.dimensions().0);
        assert!(self.size.1 + self.left_top.1 <= self.rendered.dimensions().1);

        let base_view = base.view(self.left_top.0, self.left_top.1, self.size.0, self.size.1);

        let clipped =
            self.rendered
                .view(self.left_top.0, self.left_top.1, self.size.0, self.size.1);

        let mut clipped = clipped.to_image();
        if self.dispose == DisposalMethod::Keep {
            for (clipped_pix, (_, _, base_pix)) in clipped.pixels_mut().zip(base_view.pixels()) {
                if base_pix == *clipped_pix {
                    *clipped_pix = Rgba([0, 0, 0, 0]);
                }
            }
        }

        clipped
    }

    pub fn scaled(&self, ratio: Ratio<u32>, filter: FilterType) -> RenderedFrame {
        let orig_dims = (self.rendered.width() as u32, self.rendered.height() as u32);
        let dims = scale_point(ratio, orig_dims);

        // TODO: Consider using a higher bit depth during resize to minimize color distortion,
        //       particularly around areas of transparency
        let mut rendered = image::imageops::resize(&self.rendered, dims.0, dims.1, filter);

        for pix in rendered.pixels_mut() {
            // 0x80 is an arbitrary threshold; ie: if opacity is at least 50% we'll keep the pixel
            // if pix[3] != 0 && pix[3] != 255 {
            *pix = flatten_alpha_depth(pix);
            // }
        }

        let left_top = scale_point(ratio, self.left_top);

        // Scale rounding up for the right bottom to cover partial pixels
        let right_bottom = scale_point_ceil(
            ratio,
            (self.left_top.0 + self.size.0, self.left_top.1 + self.size.1),
        );
        let right_bottom = (right_bottom.0.min(dims.0), right_bottom.1.min(dims.1));

        let size = (right_bottom.0 - left_top.0, right_bottom.1 - left_top.1);

        RenderedFrame {
            delay: self.delay,
            dispose: self.dispose,
            left_top,
            size,
            rendered,
        }
    }
}

impl<T: Read> FrameIter<T> {
    pub fn new(input: T) -> FrameIter<T> {
        let mut gif_opts = gif::DecodeOptions::new();
        gif_opts.set_color_output(gif::ColorOutput::Indexed);
        let decoder = gif_opts.read_info(input).unwrap();
        let screen = gif_dispose::Screen::new_decoder(&decoder);
        FrameIter { screen, decoder }
    }
}

impl<T: Read> Iterator for FrameIter<T> {
    type Item = RenderedFrame;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(frame) = self.decoder.read_next_frame().unwrap_or(None) {
            let img = self.screen.blit_frame(frame).unwrap();
            let pix_buf: Vec<u8> = img.into_iter().flat_map(|pix| pix.iter()).collect();
            let rendered = ImageBuffer::from_vec(
                self.screen.pixels.width() as u32,
                self.screen.pixels.height() as u32,
                pix_buf,
            )
            .unwrap();

            Some(RenderedFrame {
                delay: frame.delay,
                dispose: frame.dispose,
                left_top: (frame.left as u32, frame.top as u32),
                size: (frame.width as u32, frame.height as u32),
                rendered,
            })
        } else {
            None
        }
    }
}

/// Resize an animation
/// Currently only supports Gifs and does not crop images
pub fn resize_animation(request: ResizeRequest) -> Result<ResizeResult, MultiErr> {
    let frames = FrameIter::new(request.input.as_slice());
    let orig_size = frames.dimensions();
    let requested_size = (request.target_w as u32, request.target_h as u32);

    let ratio = if ResizeType::Fit == request.resize_op {
        Ratio::new(requested_size.0, orig_size.0).min(Ratio::new(requested_size.1, orig_size.1))
    } else {
        Ratio::new(requested_size.0, orig_size.0).max(Ratio::new(requested_size.1, orig_size.1))
    };

    let ratio = if request.down_only {
        ratio.min(Ratio::from_integer(1))
    } else {
        ratio
    };

    let out_size = scale_point(ratio, orig_size);

    // Keep at least 1 pixel:
    let out_size = (out_size.0.max(1), out_size.1.max(1));

    let mut out = vec![];
    let mut enc = gif::Encoder::new(&mut out, out_size.0 as u16, out_size.1 as u16, &[]).unwrap();

    // TODO: Use the actual value from the NETSCAPE2.0 extension.
    //       See `fn display_gif_details(...)` for PoC using the StreamingDecoder.
    enc.set_repeat(Repeat::Infinite).unwrap();

    let mut prev_screen = ImageBuffer::from_pixel(out_size.0, out_size.1, Rgba([0, 0, 0, 0]));

    for frame in frames {
        let scaled_frame = frame.scaled(ratio, request.scale_filter);
        let mut trimmed = scaled_frame.clip_changes(&prev_screen);

        let mut out_frame = gif::Frame::from_rgba_speed(
            trimmed.width() as u16,
            trimmed.height() as u16,
            trimmed.as_mut(),
            1,
        );

        out_frame.delay = frame.delay;
        out_frame.dispose = frame.dispose;

        out_frame.left = scaled_frame.left_top.0 as u16;
        out_frame.top = scaled_frame.left_top.1 as u16;
        enc.write_frame(&out_frame).unwrap();

        if frame.dispose != DisposalMethod::Previous {
            prev_screen = scaled_frame.rendered;
        }
    }
    drop(enc);

    Ok(ResizeResult {
        output: out,
        format: "GIF".into(),
        version: VERSION.into(),
        w: out_size.0 as u16,
        h: out_size.1 as u16,
    })
}

fn scale_point(ratio: Ratio<u32>, pt: (u32, u32)) -> (u32, u32) {
    ((ratio * pt.0).to_integer(), (ratio * pt.1).to_integer())
}

fn scale_point_ceil(ratio: Ratio<u32>, pt: (u32, u32)) -> (u32, u32) {
    (
        (ratio * pt.0).ceil().to_integer(),
        (ratio * pt.1).ceil().to_integer(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_adjust() {
        let black = Rgba([0, 0, 0, 255]);
        let white = Rgba([255, 255, 255, 255]);
        let gray = Rgba([128, 128, 128, 255]);
        assert_eq!(flatten_alpha_depth(&black), black);
        assert_eq!(flatten_alpha_depth(&white), white);
        assert_eq!(flatten_alpha_depth(&gray), gray);

        let transp = Rgba([0, 0, 0, 0]);
        let black49 = Rgba([0, 0, 0, 127]);
        assert_eq!(flatten_alpha_depth(&transp), transp);
        assert_eq!(flatten_alpha_depth(&black49), transp);

        let gray50 = Rgba([128, 128, 128, 128]);
        let light_gray = Rgba([192, 192, 192, 255]);
        assert_eq!(flatten_alpha_depth(&gray50), light_gray);

        let white50 = Rgba([255, 255, 255, 128]);
        // this shouldn't happen, but we should probably not crash if it does:
        assert_eq!(flatten_alpha_depth(&white50), white);
    }
}
