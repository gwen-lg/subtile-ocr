#![doc = include_str!("../README.md")]

mod ocr;
mod opt;
mod preprocessor;

use crate::opt::Opt;
use clap::Parser;
use log::{warn, LevelFilter};
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
use subparse::{timetypes::TimeSpan, SrtFile, SubtitleFile};
use subtile::SubError;
use thiserror::Error;

#[derive(Error, Debug)]
enum Error {
    #[error("Could not parse VOB subtitles from {}", path.display())]
    ReadSubtitles { path: PathBuf, source: SubError },

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

fn run(opt: Opt) -> anyhow::Result<()> {
    let vobsubs =
        preprocessor::preprocess_subtitles(&opt).map_err(|source| Error::ReadSubtitles {
            path: opt.input.clone(),
            source,
        })?;

    // Dump images if requested.
    if opt.dump {
        for (i, sub) in vobsubs.iter().enumerate() {
            for (j, image) in sub.images.iter().enumerate() {
                let filename = format!("{:06}-{:02}.png", i, j);
                image
                    .save(&filename)
                    .map_err(|source| Error::DumpImage { filename, source })?;
            }
        }
    }

    let subtitles = ocr::process(vobsubs, &opt)?;

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

    write_srt(opt.output, &subtitle_data)?;

    if ocr_error_count > 0 {
        Err(Error::OcrFails(ocr_error_count).into())
    } else {
        Ok(())
    }
}

fn write_srt(path: Option<PathBuf>, subtitle_data: &[u8]) -> Result<(), Error> {
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

fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .without_timestamps()
        .with_level(LevelFilter::Warn)
        .env()
        .init()
        .unwrap();
    let code = match run(Opt::parse()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("An error occured: {}", e);
            e.chain().for_each(|x| println!("  {x}"));
            // if let Some(backtrace) = ErrorCompat::backtrace(&e) {
            //     println!("{}", backtrace);
            // }
            1 //TODO: 1 is error ?
        }
    };
    std::process::exit(code);
}
