#![doc = include_str!("../README.md")]

mod ocr;
mod opt;
mod preprocessor;

pub use crate::ocr::{process as ocr_process, OcrOpt};
pub use crate::opt::Opt;
pub use crate::preprocessor::{preprocess_subtitles, ImagePreprocessOpt};

use log::warn;
use std::{
    fs::File,
    io::{self, BufWriter},
    path::PathBuf,
};
use subtile::{srt, time::TimeSpan, vobsub, SubError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Could not parse VOB subtitles.")]
    ReadSubtitles(#[from] SubError),

    #[error("Could not perform OCR on subtitles.")]
    Ocr(#[from] ocr::Error),

    #[error("Error happen during OCR on {0} subtitles images")]
    OcrFails(u32),

    #[error("Could not generate SRT file: {message}")]
    GenerateSrt { message: String },

    #[error("Could not write SRT file {}", path.display())]
    WriteSrtFile { path: PathBuf, source: io::Error },

    #[error("Could not write SRT on stdout.")]
    WriteSrtStdout { source: io::Error },

    #[error("Could not write image dump file '{filename}'")]
    DumpImage {
        filename: String,
        source: image::ImageError,
    },
}

#[profiling::function]
pub fn run(opt: &Opt) -> anyhow::Result<()> {
    rayon::ThreadPoolBuilder::new()
        .thread_name(|idx| format!("Rayon_{idx}"))
        .build_global() // _global
        .unwrap();

    let idx = {
        profiling::scope!("Open idx");
        vobsub::Index::open(&opt.input)?
    };
    let image_opt = ImagePreprocessOpt::new(opt.threshold, opt.border);
    let vobsubs = preprocessor::preprocess_subtitles(idx, image_opt)?;

    // Dump images if requested.
    if opt.dump {
        dump_images(&vobsubs)?;
    }

    let ocr_opt = OcrOpt::new(&opt.tessdata_dir, opt.lang.as_str(), &opt.config, opt.dpi);
    let subtitles = ocr::process(vobsubs, &ocr_opt)?;
    let subtitles = check_subtitles(subtitles)?;

    // Create subtitle file.
    write_srt(&opt.output, &subtitles)?;

    Ok(())
}

/// dump all images
#[profiling::function]
fn dump_images(vobsubs: &[preprocessor::PreprocessedVobSubtitle]) -> Result<(), Error> {
    vobsubs.iter().enumerate().try_for_each(|(i, sub)| {
        sub.images
            .iter()
            .enumerate()
            .try_for_each(|(j, image)| dump_image(i, j, image))
    })
}

/// dump one image
#[profiling::function]
fn dump_image(
    i: usize,
    j: usize,
    image: &image::ImageBuffer<image::Luma<u8>, Vec<u8>>,
) -> Result<(), Error> {
    let filename = format!("{:06}-{:02}.png", i, j);
    image
        .save(&filename)
        .map_err(|source| Error::DumpImage { filename, source })
}

/// Log errors and remove bad results.

#[profiling::function]
pub fn check_subtitles(
    subtitles: Vec<Result<(TimeSpan, String), ocr::Error>>,
) -> Result<Vec<(TimeSpan, String)>, Error> {
    let mut ocr_error_count = 0;
    let subtitles: Vec<(TimeSpan, String)> = subtitles
        .into_iter()
        .filter_map(|maybe_subtitle| match maybe_subtitle {
            Ok(subtitle) => Some(subtitle),
            Err(e) => {
                warn!("Error while running OCR on subtitle image: {}", e);
                ocr_error_count += 1;
                None
            }
        })
        .collect();

    if ocr_error_count > 0 {
        Err(Error::OcrFails(ocr_error_count))
    } else {
        Ok(subtitles)
    }
}

#[profiling::function]
fn write_srt(path: &Option<PathBuf>, subtitles: &[(TimeSpan, String)]) -> Result<(), Error> {
    match &path {
        Some(path) => {
            let mkerr = |source| Error::WriteSrtFile {
                path: path.to_path_buf(),
                source,
            };

            // Write to file.
            let subtitle_file = File::create(path).map_err(mkerr)?;
            let mut stream = BufWriter::new(subtitle_file);
            srt::write_srt(subtitles, &mut stream).map_err(mkerr)?;
        }
        None => {
            // Write to stdout.
            let mut stdout = io::stdout();
            srt::write_srt(subtitles, &mut stdout)
                .map_err(|source| Error::WriteSrtStdout { source })?;
        }
    }
    Ok(())
}
