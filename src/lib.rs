#![doc = include_str!("../README.md")]

mod glyph;
mod ocr;
mod ocs;
mod opt;
mod preprocessor;

pub use crate::{ocr::OcrOpt, opt::Opt};

use glyph::GlyphLibrary;
use image::{GrayImage, LumaA};
use log::warn;
use ocs::ImagePieces;
use preprocessor::rgb_palette_to_luminance;
use ratatui::{prelude::Backend, Terminal};
use rayon::{
    iter::{IntoParallelRefIterator, ParallelIterator},
    ThreadPoolBuildError,
};
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, BufReader, BufWriter},
    path::PathBuf,
};
use subtile::{
    image::{dump_images, luma_a_to_luma, ToImage, ToOcrImage, ToOcrImageOpt},
    pgs::{self, DecodeTimeImage, RleToImage},
    srt,
    time::TimeSpan,
    vobsub::{self, conv_to_rgba, VobSubError, VobSubIndexedImage, VobSubOcrImage, VobSubToImage},
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

    #[error("The file doesn't have a valid extension, can't choose a parser.")]
    NoFileExtension,

    #[error("Failed to open Index file.")]
    IndexOpen(#[source] VobSubError),

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

    #[error("Failed to split subtitle image into character image")]
    OcsSplit(#[source] ocs::Error),

    #[error("Failed to extract text of the split subtitle image")]
    OcsConvertToText(#[source] ocs::Error),

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
/// Will return [`Error::WriteSrtFile`] of [`Error::WriteSrtStdout`] if failed to write subtitles as `srt`.
/// Will forward error from `ocr` processing and [`check_subtitles`] if any.
#[profiling::function]
pub fn run(opt: &Opt, terminal: Terminal<impl Backend>) -> Result<(), Error> {
    rayon::ThreadPoolBuilder::new()
        .thread_name(|idx| format!("Rayon_{idx}"))
        .build_global()
        .map_err(Error::RayonThreadPool)?;

    let (times, images) = match opt.input.extension().and_then(OsStr::to_str) {
        Some(ext) => match ext {
            "sup" => process_pgs(opt),
            "idx" => process_vobsub(opt),
            ext => Err(Error::InvalidFileExtension {
                extension: ext.into(),
            }),
        },
        None => Err(Error::NoFileExtension),
    }?;

    // Dump images if requested.
    if opt.dump {
        dump_images("dumps", &images).map_err(Error::DumpImage)?;
    }

    let mut glyph_lib = GlyphLibrary::new();

    // test image split
    let test_img_iter = images.iter().skip(9).take(5); //TODO: remove
    dump_images("dump", test_img_iter.clone()).map_err(Error::DumpImage)?;
    let texts = test_img_iter
        .into_iter()
        .enumerate()
        .map(|(idx, img)| {
            let splitter = ocs::ImageCharacterSplitter::from_image(img);
            let pieces = splitter.split_in_character_img().map_err(Error::OcsSplit)?;
            dump_sub_pieces(idx, &pieces)?;

            pieces
                .process_to_text(&mut glyph_lib)
                .map_err(Error::OcsConvertToText)
        })
        .collect::<Vec<_>>();

    //let ocr_opt = OcrOpt::new(&opt.tessdata_dir, opt.lang.as_str(), &opt.config, opt.dpi);
    //let texts = ocr::process(images, &ocr_opt)?;
    let subtitles = check_subtitles(times.into_iter().zip(texts))?;

    // Create subtitle file.
    write_srt(&opt.output, &subtitles)?;

    Ok(())
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
            .map(|rle_img| RleToImage::new(rle_img, |pix: LumaA<u8>| pix).to_image());
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
        let palette = rgb_palette_to_luminance(idx.palette());
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
pub fn check_subtitles<In, InErr>(subtitles: In) -> Result<Vec<(TimeSpan, String)>, Error>
where
    In: IntoIterator<Item = (TimeSpan, Result<String, InErr>)>,
    InErr: Into<Error> + std::error::Error + Send + Sync + 'static,
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

// Dump images for subtitle pieces.
// This can help for debug.
fn dump_sub_pieces(idx: usize, pieces: &ImagePieces) -> Result<(), Error> {
    pieces
        .images()
        .enumerate()
        .try_for_each(|(line_idx, images)| {
            let foldername = format!("dumpsplit_{idx}_{line_idx}");
            dump_images(foldername.as_str(), images).map_err(Error::DumpImage)
        })
}
