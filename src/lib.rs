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
use subtile::{image::dump_images, srt, time::TimeSpan, vobsub, SubError};
use thiserror::Error;

/// Gather different `Error`s in a dedicated enum.
#[allow(missing_docs)]
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
}

/// Run OCR for `opt`.
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
        let images = vobsubs.iter().map(|sub| &sub.image);
        dump_images("dumps", images)?;
    }

    let ocr_opt = OcrOpt::new(&opt.tessdata_dir, opt.lang.as_str(), &opt.config, opt.dpi);
    let subtitles = ocr::process(vobsubs, &ocr_opt)?;
    let subtitles = check_subtitles(subtitles)?;

    // Create subtitle file.
    write_srt(&opt.output, &subtitles)?;

    Ok(())
}

/// Log errors and remove bad results.
#[profiling::function]
pub fn check_subtitles<In>(subtitles: In) -> Result<Vec<(TimeSpan, String)>, Error>
where
    In: IntoIterator<Item = Result<(TimeSpan, String), ocr::Error>>,
{
    let mut ocr_error_count = 0;
    let subtitles: Vec<(TimeSpan, String)> = subtitles
        .into_iter()
        .enumerate()
        .filter_map(|(idx, maybe_subtitle)| match maybe_subtitle {
            Ok(subtitle) => Some(subtitle),
            Err(e) => {
                let err = anyhow::Error::new(e); // warp in anyhow::Error to display the error stack with :#
                warn!(
                    "Error while running OCR on subtitle image ({}):\n\t {:#}",
                    idx + 1,
                    err
                );
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
