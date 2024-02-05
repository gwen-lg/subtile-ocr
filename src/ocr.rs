use std::{io::Cursor, str::Utf8Error};

use crate::preprocessor::PreprocessedVobSubtitle;
use image::{DynamicImage, GrayImage};
use leptess::{
    leptonica::PixError,
    tesseract::{TessInitError, TessSetVariableError},
    LepTess, Variable,
};
use rayon::prelude::*;
use scoped_tls_hkt::scoped_thread_local;
use subtile::time::TimeSpan;
use thiserror::Error;

scoped_thread_local!(static mut TESSERACT: Option<TesseractWrapper>);

/// Options for orc with Tesseract
pub struct OcrOpt<'a> {
    tessdata_dir: &'a Option<String>,
    lang: &'a str,
    config: &'a Vec<(Variable, String)>,
    dpi: i32,
}

impl<'a> OcrOpt<'a> {
    /// Create a new `OcrOpt`
    #[must_use]
    pub fn new(
        tessdata_dir: &'a Option<String>,
        lang: &'a str,
        config: &'a Vec<(Variable, String)>,
        dpi: i32,
    ) -> Self {
        Self {
            tessdata_dir,
            lang,
            config,
            dpi,
        }
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Could not build tesseract thread pool")]
    BuildThreadPool(#[from] rayon::ThreadPoolBuildError),

    #[error("Could not initialize tesseract")]
    Initialize(#[from] TessInitError),

    #[error("Could not set tesseract variable")]
    SetVariable(#[from] TessSetVariableError),

    #[error("Could not write image to memory")]
    WriteImage(#[from] image::ImageError),

    #[error("Could not set tesseract image")]
    SetImage(#[from] PixError),

    #[error("Could not get tesseract text")]
    GetText(#[from] Utf8Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Process OCR for subtitle images.
#[profiling::function]
pub fn process(
    vobsubs: Vec<PreprocessedVobSubtitle>,
    opt: &OcrOpt,
) -> Result<Vec<Result<(TimeSpan, String)>>> {
    std::env::set_var("OMP_THREAD_LIMIT", "1");
    let subs = rayon::ThreadPoolBuilder::new().build_scoped(
        |thread| {
            let mut tesseract = None;
            TESSERACT.set(&mut tesseract, || thread.run())
        },
        |pool| {
            pool.install(|| {
                vobsubs
                    .into_par_iter()
                    .map(|vobsub| {
                        let text = TESSERACT.with(|maybe_tesseract| {
                            profiling::scope!("tesseract_ocr");
                            let tesseract = match maybe_tesseract {
                                Some(tesseract) => tesseract,
                                None => {
                                    let tesseract = TesseractWrapper::new(
                                        opt.tessdata_dir.as_deref(),
                                        opt.lang,
                                        opt.config,
                                    )?;
                                    maybe_tesseract.insert(tesseract)
                                }
                            };
                            tesseract.set_image(vobsub.image, opt.dpi)?;
                            tesseract.get_text()
                        })?;
                        Ok((vobsub.time_span, text))
                    })
                    .collect::<Vec<Result<(TimeSpan, String)>>>()
            })
        },
    )?;
    Ok(subs)
}

struct TesseractWrapper {
    leptess: LepTess,
}

impl TesseractWrapper {
    fn new(
        datapath: Option<&str>,
        language: impl AsRef<str>,
        config: &[(Variable, String)],
    ) -> Result<Self> {
        profiling::scope!("TesseractWrapper new");

        let mut leptess = LepTess::new(datapath, language.as_ref())?;
        // Disable learning by default, though a user could re-enable this
        // option with `-c`. We turn this off since we are are multithreading,
        // so this option would result in non-deterministic output.
        leptess.set_variable(leptess::Variable::ClassifyEnableLearning, "0")?;
        // 6 is PSM_SINGLE_BLOCK. We have preprocessed the input into individual
        // lines, and telling Tesseract this fact greatly improves accuracy.
        leptess.set_variable(leptess::Variable::TesseditPagesegMode, "6")?;
        // Avoid interpreting the characters I, l as |
        leptess.set_variable(leptess::Variable::TesseditCharBlacklist, "|")?;
        // Add user options.
        for (key, value) in config {
            leptess.set_variable(*key, value)?;
        }
        Ok(Self { leptess })
    }

    /// Set the tesseract image to the given image's contents.
    #[profiling::function]
    fn set_image(&mut self, image: GrayImage, dpi: i32) -> Result<()> {
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(image).write_to(&mut bytes, image::ImageFormat::Pnm)?;
        self.leptess.set_image_from_mem(bytes.get_ref())?;
        self.leptess.set_source_resolution(dpi);
        Ok(())
    }

    /// Get text.
    #[profiling::function]
    fn get_text(&mut self) -> Result<String> {
        Ok(self.leptess.get_utf8_text()?)
    }
}
