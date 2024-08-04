#![doc = include_str!("../README.md")]

mod ocr;
mod opt;
mod preprocessor;

pub use crate::{ocr::OcrOpt, opt::Opt};

use log::warn;
use preprocessor::rgb_palette_to_luminance;
use rayon::{
    iter::{IntoParallelRefIterator, ParallelIterator},
    ThreadPoolBuildError,
};
use std::{
    fs::File,
    io::{self, BufWriter},
    path::PathBuf,
};
use subtile::{
    image::{dump_images, ToOcrImage, ToOcrImageOpt},
    srt,
    time::TimeSpan,
    vobsub::{self, VobSubError, VobSubIndexedImage, VobSubOcrImage},
    SubtileError,
};
use thiserror::Error;

/// Gather different `Error`s in a dedicated enum.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to create a rayon ThreadPool.")]
    RayonThreadPool(#[from] ThreadPoolBuildError),

    #[error("Could not parse VOB subtitles.")]
    ReadSubtitles(#[from] SubtileError),

    #[error("Failed to open Index file.")]
    IndexOpen(#[source] VobSubError),

    #[error("Failed to dump subtitles images")]
    DumpImage(#[source] SubtileError),

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
}

/// Run OCR for `opt`.
///
/// # Errors
///
/// Will return [`Error::RayonThreadPool`] if `build_global` of the `ThreadPool` rayon failed.
/// Will return [`Error::IndexOpen`] if the subtitle files can't be opened.
/// Will return [`Error::DumpImage`] if an error occurred during dump.
#[profiling::function]
pub fn run(opt: &Opt) -> Result<(), Error> {
    rayon::ThreadPoolBuilder::new()
        .thread_name(|idx| format!("Rayon_{idx}"))
        .build_global()
        .map_err(Error::RayonThreadPool)?;

    let idx = {
        profiling::scope!("Open idx");
        vobsub::Index::open(&opt.input).map_err(Error::IndexOpen)?
    };
    let (times, images): (Vec<_>, Vec<_>) = {
        profiling::scope!("Parse subtitles");
        idx.subtitles::<(TimeSpan, VobSubIndexedImage)>()
            .filter_map(|sub| match sub {
                Ok(sub) => Some(sub),
                Err(e) => {
                    warn!(
                    "warning: unable to read subtitle: {e}. (This can usually be safely ignored.)"
                );
                    None
                }
            })
            .unzip()
    };

    let images_for_ocr = {
        profiling::scope!("Convert images for OCR");

        let ocr_opt = ocr_opt(opt);
        let palette = rgb_palette_to_luminance(idx.palette());
        images
            .par_iter()
            .map(|vobsub_img| {
                let converter = VobSubOcrImage::new(vobsub_img, &palette);
                converter.image(&ocr_opt)
            })
            .collect::<Vec<_>>()
    };

    // Dump images if requested.
    if opt.dump {
        dump_images("dumps", &images_for_ocr).map_err(Error::DumpImage)?;
    }

    let ocr_opt = OcrOpt::new(&opt.tessdata_dir, opt.lang.as_str(), &opt.config, opt.dpi);
    let texts = ocr::process(images_for_ocr, &ocr_opt)?;
    let subtitles = check_subtitles(times.into_iter().zip(texts))?;

    // Create subtitle file.
    write_srt(&opt.output, &subtitles)?;

    Ok(())
}

/// Create [`ToOcrImageOpt`] from [`Opt`]
fn ocr_opt(opt: &Opt) -> ToOcrImageOpt {
    ToOcrImageOpt {
        border: opt.border,
        ..Default::default()
    }
}

/// Log errors and remove bad results.
///
/// # Errors
///  Will return [`Error::OcrFails`] if the ocr return an error for at least one image.
#[profiling::function]
pub fn check_subtitles<In>(subtitles: In) -> Result<Vec<(TimeSpan, String)>, Error>
where
    In: IntoIterator<Item = (TimeSpan, Result<String, ocr::Error>)>,
{
    let mut ocr_error_count = 0;
    let subtitles = subtitles
        .into_iter()
        .enumerate()
        .filter_map(|(idx, (time, maybe_text))| match maybe_text {
            Ok(text) => Some((time, text)),
            Err(e) => {
                let err = anyhow::Error::new(e); // warp in anyhow::Error to display the error stack with :#
                warn!(
                    "Error while running OCR on subtitle image ({} - {time:?}):\n\t {err:#}",
                    idx + 1,
                );
                ocr_error_count += 1;
                None
            }
        })
        .collect::<Vec<_>>();

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
            srt::write_srt(&mut stream, subtitles).map_err(mkerr)?;
        }
        None => {
            // Write to stdout.
            let mut stdout = io::stdout();
            srt::write_srt(&mut stdout, subtitles)
                .map_err(|source| Error::WriteSrtStdout { source })?;
        }
    }
    Ok(())
}
