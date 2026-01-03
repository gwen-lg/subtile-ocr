use std::{cell::RefCell, io::Cursor, str::Utf8Error};

use image::{DynamicImage, GrayImage};
use leptess::{
    LepTess, Variable,
    leptonica::PixError,
    tesseract::{TessInitError, TessSetVariableError},
};
use log::trace;
use rayon::{broadcast, prelude::*};
use thiserror::Error;

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
    pub const fn new(
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

/// Error of the Ocr process with Tesseract.
#[derive(Error, Debug)]
pub enum Error {
    /// Indicate than `Tesseract` could not be initialized.
    #[error("could not initialize tesseract")]
    Initialize(#[from] TessInitError),

    /// Indicate than `TESSERACT` was already initialized on this thread
    #[error("thread local var `TESSERACT` is already initialized")]
    AlreadyInitialized,

    /// Indicate an error during `Tesseract` variable set.
    #[error("could not set tesseract variable")]
    SetVariable(#[from] TessSetVariableError),

    /// Indicate than the `pnm` image couldn't be wrote in memory.
    #[error("could not write image to memory")]
    WritePnmImage(#[from] image::ImageError),

    /// Indicate a failure during set `Pnm` image to `Tesseract`.
    #[error("could not set `Tesseract` image")]
    SetImage(#[from] PixError),

    /// Indicate than `Tesseract` failed to provide a text from the image.
    #[error("could not get `Tesseract` text")]
    GetText(#[from] Utf8Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

thread_local! {
    static TESSERACT: RefCell<Option<TesseractWrapper>> = const { RefCell::new(None) };
}

/// Process subtitles images with Tesseract `OCR`.
#[profiling::function]
pub fn process<Img>(images: Img, opt: &OcrOpt) -> Result<Vec<Result<String>>>
where
    Img: IntoParallelIterator<Item = GrayImage>,
{
    // SAFETY:
    // As env var is set before initialize TesseractWrapper,
    // It should be okay to set the environment variable `OMP_THREAD_LIMIT`.
    unsafe { std::env::set_var("OMP_THREAD_LIMIT", "1") };

    // Init tesseract on the main thread:
    let tesseract = TesseractWrapper::new(opt.tessdata_dir.as_deref(), opt.lang, opt.config)?;
    if TESSERACT.replace(Some(tesseract)).is_some() {
        return Err(Error::AlreadyInitialized);
    }
    // and on threadpool:
    broadcast(|ctx| {
        profiling::scope!("Tesseract Init Wrapper");
        trace!(
            "Init tesseract with lang `{}` on thread {}",
            opt.lang,
            ctx.index()
        );
        let tesseract = TesseractWrapper::new(opt.tessdata_dir.as_deref(), opt.lang, opt.config)?;
        if TESSERACT.replace(Some(tesseract)).is_some() {
            return Err(Error::AlreadyInitialized);
        }
        Ok::<_, Error>(())
    })
    .into_iter()
    .try_for_each(|init_res| init_res)?;

    // Process images
    let subs = images
        .into_par_iter()
        .map(|image| {
            let text = TESSERACT.with(|tesseract| {
                profiling::scope!("tesseract_ocr");
                let mut tesseract = tesseract.borrow_mut();
                let tesseract = tesseract.as_mut().unwrap();
                tesseract.set_image(image, opt.dpi)?;
                tesseract.get_text()
            })?;
            Ok(text)
        })
        .collect::<Vec<Result<String>>>();

    // Clean tesseract from Thread local vars for Threadpool
    broadcast(|ctx| {
        profiling::scope!("Tesseract Drop Wrapper");
        trace!("Drop TesseractWrapper local var on thread {}", ctx.index());
        if let Some(tesseract) = TESSERACT.take() {
            drop(tesseract);
        }
    });
    // ... for main thread
    if let Some(tesseract) = TESSERACT.take() {
        drop(tesseract);
    }

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
        leptess.set_variable(leptess::Variable::TesseditCharBlacklist, "|[]")?;
        // Avoid than tesseract tried to invert the image
        leptess.set_variable(leptess::Variable::TesseditDoInvert, "0")?;
        // Add user options.
        for (key, value) in config {
            leptess.set_variable(*key, value)?;
        }
        Ok(Self { leptess })
    }

    /// Set the tesseract image to the given image's contents.
    #[profiling::function]
    fn set_image(&mut self, image: GrayImage, dpi: i32) -> Result<()> {
        let bytes = {
            profiling::scope!("TesseractWrapper Pnm create");
            let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            DynamicImage::ImageLuma8(image).write_to(&mut bytes, image::ImageFormat::Pnm)?;
            bytes
        };
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
