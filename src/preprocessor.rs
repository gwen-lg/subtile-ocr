use image::{GrayImage, ImageBuffer, Luma};
use iter_fixed::IntoIteratorFixed;
use log::warn;
use rayon::prelude::*;
use subtile::{
    time::{TimePoint, TimeSpan},
    vobsub, SubError,
};

/// Option for Image preprocessing.
pub struct ImagePreprocessOpt {
    threshold: f32,
    border: u32,
}

impl ImagePreprocessOpt {
    /// Create new `ImagePreprocessOpt`
    #[must_use]
    pub fn new(threshold: f32, border: u32) -> Self {
        Self { threshold, border }
    }
}

pub struct PreprocessedVobSubtitle {
    pub time_span: TimeSpan,
    pub force: bool,
    pub image: GrayImage,
}

/// Return a vector of binarized subtitles.
#[profiling::function]
pub fn preprocess_subtitles(
    idx: vobsub::Index,
    opt: ImagePreprocessOpt,
) -> Result<Vec<PreprocessedVobSubtitle>, SubError> {
    let subtitles: Vec<vobsub::Subtitle> = {
        profiling::scope!("Parse subtitles");
        idx.subtitles()
            .filter_map(|sub| match sub {
                Ok(sub) => Some(sub),
                Err(e) => {
                    warn!(
                    "warning: unable to read subtitle: {}. (This can usually be safely ignored.)",
                    e
                );
                    None
                }
            })
            .collect()
    };
    let palette = rgb_palette_to_luminance(idx.palette());
    let result = subtitles
        .par_iter()
        .filter_map(|sub| {
            subtitle_to_image(sub, &palette, opt.threshold, opt.border).map(|image| {
                PreprocessedVobSubtitle {
                    time_span: TimeSpan::new(
                        seconds_to_time_point(sub.start_time()),
                        seconds_to_time_point(sub.end_time()),
                    ),
                    force: sub.force(),
                    image,
                }
            })
        })
        .collect();
    Ok(result)
}

fn seconds_to_time_point(seconds: f64) -> TimePoint {
    TimePoint::from_msecs((seconds * 1000.0) as i64)
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

/// Given a subtitle, binarize, invert, and add a border
/// for direct feeding into Tesseract.
#[profiling::function]
fn subtitle_to_image(
    subtitle: &vobsub::Subtitle,
    palette: &[f32; 16],
    threshold: f32,
    border: u32,
) -> Option<GrayImage> {
    let sub_palette_visibility = generate_visibility_palette(subtitle);

    let binarized_palette = binarize_palette(
        palette,
        subtitle.palette(),
        &sub_palette_visibility,
        threshold,
    );

    let width = u32::from(subtitle.area().width());
    let height = u32::from(subtitle.area().height());

    let image = ImageBuffer::from_fn(width + border * 2, height + border * 2, |x, y| {
        if x < border || x >= width + border || y < border || y >= height + border {
            Luma([255])
        } else {
            let offset = (y - border) * width + (x - border);
            let sub_palette_ix = subtitle.raw_image()[offset as usize] as usize;
            if binarized_palette[sub_palette_ix] {
                Luma([0])
            } else {
                Luma([255])
            }
        }
    });
    Some(image)
}

/// Find all the palette indices used in this image, and filter out the
/// transparent ones. Checking each and every single pixel in the image like
/// this is probably not strictly necessary, but it could theoretically catch an
/// edge case.
#[profiling::function]
fn generate_visibility_palette(subtitle: &vobsub::Subtitle) -> [bool; 4] {
    let mut sub_palette_visibility =
        subtitle
            .raw_image()
            .iter()
            .fold([false; 4], |mut visible: [bool; 4], &sub_palette_ix| {
                visible[sub_palette_ix as usize] = true;
                visible
            });
    // The alpha palette is reversed.
    for (i, &alpha) in subtitle.alpha().iter().rev().enumerate() {
        if alpha == 0 {
            sub_palette_visibility[i] = false;
        }
    }
    sub_palette_visibility
}

/// Generate a binarized palette where `true` represents a filled text pixel.
#[profiling::function]
fn binarize_palette(
    palette: &[f32; 16],
    sub_palette: &[u8; 4],
    sub_palette_visibility: &[bool; 4],
    threshold: f32,
) -> [bool; 4] {
    // Find the max luminance, so we can scale each luminance value by it.
    // Reminder that the sub palette is reversed.
    let mut max_luminance = 0.0;
    for (&palette_ix, &visible) in sub_palette.iter().rev().zip(sub_palette_visibility) {
        if visible {
            let luminance = palette[palette_ix as usize];
            if luminance > max_luminance {
                max_luminance = luminance;
            }
        }
    }

    // Empty image?
    if max_luminance == 0.0 {
        return [false; 4];
    }

    sub_palette
        .into_iter_fixed()
        .rev()
        .zip(sub_palette_visibility)
        .map(|(&palette_ix, &visible)| {
            if visible {
                let luminance = palette[palette_ix as usize] / max_luminance;
                luminance > threshold
            } else {
                false
            }
        })
        .collect()
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
