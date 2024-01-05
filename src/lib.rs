#![doc = include_str!("../README.md")]

mod ocr;
mod opt;
mod preprocessor;

pub use crate::ocr::process as ocr_process;
pub use crate::opt::Opt;
pub use crate::preprocessor::{preprocess_subtitles, ImagePreprocessOpt};

use log::warn;
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use subparse::{timetypes::TimeSpan, SrtFile, SubtitleFile};
use subtile::{vobsub, SubError};
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

pub fn run(opt: &Opt) -> anyhow::Result<()> {
    let idx = vobsub::Index::open(&opt.input)?;
    let image_opt = ImagePreprocessOpt::new(opt.threshold, opt.border);
    let vobsubs = preprocessor::preprocess_subtitles(idx, image_opt)?;

    // Dump images if requested.
    if opt.dump {
        dump_images(&vobsubs)?;
    }

    let subtitles = ocr::process(vobsubs, opt)?;

    // Log errors and remove bad results.
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

    // Create subtitle file.
    let subtitles =
        SubtitleFile::SubRipFile(SrtFile::create(subtitles).map_err(|e| Error::GenerateSrt {
            message: e.to_string(),
        })?);
    let subtitle_data = subtitles.to_data().map_err(|e| Error::GenerateSrt {
        message: e.to_string(),
    })?;

    write_srt(&opt.output, &subtitle_data)?;

    if ocr_error_count > 0 {
        Err(Error::OcrFails(ocr_error_count).into())
    } else {
        Ok(())
    }
}

fn dump_images(vobsubs: &[preprocessor::PreprocessedVobSubtitle]) -> Result<(), Error> {
    for (i, sub) in vobsubs.iter().enumerate() {
        for (j, image) in sub.images.iter().enumerate() {
            let filename = format!("{:06}-{:02}.png", i, j);
            image
                .save(&filename)
                .map_err(|source| Error::DumpImage { filename, source })?;
        }
    }
    Ok(())
}

fn write_srt(path: &Option<PathBuf>, subtitle_data: &[u8]) -> Result<(), Error> {
    match &path {
        Some(path) => {
            let mkerr = |source| Error::WriteSrtFile {
                path: path.to_path_buf(),
                source,
            };

            // Write to file.
            let mut subtitle_file = File::create(path).map_err(mkerr)?;
            subtitle_file.write_all(subtitle_data).map_err(mkerr)?;
        }
        None => {
            // Write to stdout.
            io::stdout()
                .write_all(subtitle_data)
                .map_err(|source| Error::WriteSrtStdout { source })?;
        }
    }
    Ok(())
}
