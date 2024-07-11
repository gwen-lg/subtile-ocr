use image::GrayImage;
use rayon::prelude::*;
use subtile::{
    image::{ToOcrImage, ToOcrImageOpt},
    vobsub::{self, VobSubIndexedImage, VobSubOcrImage},
    SubtileError,
};

/// Return a vector of processed images for OCR.
#[profiling::function]
pub fn process_images_for_ocr<I>(
    idx: vobsub::Index,
    images: I,
    border: u32,
) -> Result<Vec<GrayImage>, SubtileError>
where
    I: IntoParallelIterator<Item = VobSubIndexedImage>,
{
    let opt = ToOcrImageOpt {
        border,
        ..Default::default()
    };
    let palette = rgb_palette_to_luminance(idx.palette());
    let result = images
        .into_par_iter()
        .map(|vobsub_img| {
            let converter = VobSubOcrImage::new(&vobsub_img, &palette);
            converter.image(&opt)
        })
        .collect();
    Ok(result)
}

/// Convert an sRGB palette to a luminance palette.
fn rgb_palette_to_luminance(palette: &vobsub::Palette) -> [f32; 16] {
    palette.map(|x| {
        let r = srgb_to_linear(x[0]);
        let g = srgb_to_linear(x[1]);
        let b = srgb_to_linear(x[2]);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    })
}

/// Convert an sRGB color space channel to linear.
fn srgb_to_linear(channel: u8) -> f32 {
    let value = f32::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
