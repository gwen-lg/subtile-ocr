#![doc = include_str!("../README.md")]

mod ocr;
mod opt;

pub use crate::{ocr::process, ocr::Error as OcrError, ocr::OcrOpt, opt::Opt};

use image::GrayImage;
use log::warn;
use rayon::{
    iter::{IntoParallelRefIterator, ParallelIterator},
    ThreadPoolBuildError,
};
use std::{
    fs::File,
    io::{self, BufReader, BufWriter},
    path::{Path, PathBuf},
};
use subtile::{
    image::{dump_images, luma_a_to_luma, ToImage, ToOcrImage, ToOcrImageOpt},
    pgs::{self, DecodeTimeImage, RleToImage},
    srt,
    time::TimeSpan,
    vobsub::{
        self, conv_to_rgba, palette_rgb_to_luminance, VobSubError, VobSubIndexedImage,
        VobSubOcrImage, VobSubToImage,
    },
    SubtileError,
};
use thiserror::Error;

/// Gather different `Error`s in a dedicated enum.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to create a rayon ThreadPool.")]
    RayonThreadPool(#[from] ThreadPoolBuildError),

    #[error("The file extension '{extension}' is not managed.")]
    InvalidFileExtension { extension: String },

    #[error("The file doesn't have an extension, can't choose a parser.")]
    NoFileExtension,

    #[error("The file doesn't have an utf8 extension, can't choose a parser.")]
    NotUtf8Extension,

    #[error("Failed to open `Index` file.")]
    IndexOpen(#[source] VobSubError),

    #[error("Failed to open `Sub` file.")]
    SubOpen(#[source] VobSubError),

    #[error("Failed to create PgsParser from file")]
    PgsParserFromFile(#[source] pgs::PgsError),

    #[error("Failed to parse Pgs")]
    PgsParsing(#[source] pgs::PgsError),

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
/// Will return [`Error::InvalidFileExtension`] if the file extension is not managed.
/// Will return [`Error::NoFileExtension`] if the file have no extension.
/// Will return [`Error::NotUtf8Extension`] if the file have an extension which is not utf8.
/// Will return [`Error::WriteSrtFile`] of [`Error::WriteSrtStdout`] if failed to write subtitles as `srt`.
/// Will forward error from `ocr` processing and [`check_subtitles`] if any.
#[profiling::function]
pub fn run(opt: &Opt) -> Result<(), Error> {
    rayon::ThreadPoolBuilder::new()
        .thread_name(|idx| format!("Rayon_{idx}"))
        .build_global()
        .map_err(Error::RayonThreadPool)?;

    let (times, images) = match extract_extension(&opt.input)? {
        "sup" => process_pgs(opt),
        "sub" | "idx" => process_vobsub(opt),
        ext => Err(Error::InvalidFileExtension {
            extension: ext.into(),
        }),
    }?;

    // Dump images if requested.
    if opt.dump {
        dump_images("dumps", &images).map_err(Error::DumpImage)?;
    }

    let ocr_opt = OcrOpt::new(&opt.tessdata_dir, opt.lang.as_str(), &opt.config, opt.dpi);
    let texts = ocr::process(images, &ocr_opt)?;
    let subtitles = check_subtitles(times.into_iter().zip(texts))?;

    // Create subtitle file.
    write_srt(&opt.output, &subtitles)?;

    Ok(())
}

/// Extract extension of a path
///
/// # Errors
///
/// Will return [`Error::NoFileExtension`] if the file have no extension.
/// Will return [`Error::NotUtf8Extension`] if the file have an extension which is not utf8.
pub fn extract_extension(path: &Path) -> Result<&str, Error> {
    path.extension()
        .ok_or(Error::NoFileExtension)?
        .to_str()
        .ok_or(Error::NotUtf8Extension)
}

/// Process `PGS` subtitle file
///
/// # Errors
///
/// Will return [`Error::PgsParserFromFile`] if SupParser failed to be init from file.
/// Will return [`Error::PgsParsing`] if the parsing of subtitles failed.
/// Will return [`Error::DumpImage`] if the dump of raw image failed.
#[profiling::function]
pub fn process_pgs(opt: &Opt) -> Result<(Vec<TimeSpan>, Vec<GrayImage>), Error> {
    let parser = {
        profiling::scope!("Create PGS parser");
        subtile::pgs::SupParser::<BufReader<File>, DecodeTimeImage>::from_file(&opt.input)
            .map_err(Error::PgsParserFromFile)?
    };

    let (times, rle_images) = {
        profiling::scope!("Parse PGS file");
        parser
            .collect::<Result<(Vec<_>, Vec<_>), _>>()
            .map_err(Error::PgsParsing)?
    };

    if opt.dump_raw {
        let images = rle_images
            .iter()
            .map(|rle_img| RleToImage::new(rle_img, |pix| pix).to_image());
        dump_images("dumps_raw", images).map_err(Error::DumpImage)?;
    }

    let conv_fn = luma_a_to_luma::<_, _, 100, 100>; // Hardcoded value for alpha and luma threshold than work not bad.

    let images = {
        profiling::scope!("Convert images for OCR");
        let ocr_opt = ocr_opt(opt);
        rle_images
            .par_iter()
            .map(|rle_img| RleToImage::new(rle_img, &conv_fn).image(&ocr_opt))
            .collect::<Vec<_>>()
    };

    Ok((times, images))
}

/// Process `VobSub` subtitle file
///
/// # Errors
///
/// Will return [`Error::IndexOpen`] if the subtitle files can't be opened.
/// Will return [`Error::DumpImage`] if the dump of raw image failed.
#[profiling::function]
pub fn process_vobsub(opt: &Opt) -> Result<(Vec<TimeSpan>, Vec<GrayImage>), Error> {
    let mut input_path = opt.input.clone();
    let sub = {
        profiling::scope!("Open sub");
        input_path.set_extension("sub");
        vobsub::Sub::open(&input_path).map_err(Error::SubOpen)?
    };
    let idx = {
        profiling::scope!("Open idx");
        input_path.set_extension("idx");
        vobsub::Index::open(&input_path).map_err(Error::IndexOpen)?
    };
    let (times, images): (Vec<_>, Vec<_>) = {
        profiling::scope!("Parse subtitles");
        sub.subtitles::<(TimeSpan, VobSubIndexedImage)>()
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

    if opt.dump_raw {
        let images = images.iter().map(|rle_img| {
            let image: image::RgbaImage =
                VobSubToImage::new(rle_img, idx.palette(), conv_to_rgba).to_image();
            image
        });
        dump_images("dumps_raw", images).map_err(Error::DumpImage)?;
    }

    let images_for_ocr = {
        profiling::scope!("Convert images for OCR");

        let ocr_opt = ocr_opt(opt);
        //let palette = rgb_palette_to_luminance(idx.palette());
        let palette = palette_rgb_to_luminance(idx.palette());
        images
            .par_iter()
            .map(|vobsub_img| {
                let converter = VobSubOcrImage::new(vobsub_img, &palette);
                converter.image(&ocr_opt)
            })
            .collect::<Vec<_>>()
    };

    Ok((times, images_for_ocr))
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
