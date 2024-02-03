use std::{io::Cursor, str::Utf8Error};

use crate::{opt::Opt, preprocessor::PreprocessedVobSubtitle};
use image::{
    codecs::pnm::{PnmSubtype, SampleEncoding},
    DynamicImage, GrayImage,
};
use leptess::{
    leptonica::PixError,
    tesseract::{TessInitError, TessSetVariableError},
    LepTess, Variable,
};
use rayon::prelude::*;
use scoped_tls_hkt::scoped_thread_local;
use subparse::timetypes::TimeSpan;
use thiserror::Error;

scoped_thread_local!(static mut TESSERACT: Option<TesseractWrapper>);

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

pub fn process(
    vobsubs: Vec<PreprocessedVobSubtitle>,
    opt: &Opt,
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
                        let text = vobsub
                            .images
                            .into_iter()
                            .map(|image| {
                                TESSERACT.with(|maybe_tesseract| {
                                    let tesseract = match maybe_tesseract {
                                        Some(tesseract) => tesseract,
                                        None => {
                                            let tesseract = TesseractWrapper::new(
                                                opt.tessdata_dir.as_deref(),
                                                &opt.lang,
                                                &opt.config,
                                            )?;
                                            maybe_tesseract.insert(tesseract)
                                        }
                                    };
                                    tesseract.set_image(image, opt.dpi)?;
                                    tesseract.get_text()
                                })
                            })
                            .collect::<Result<String>>()?;
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
        let mut leptess = LepTess::new(datapath, language.as_ref())?;
        // Disable learning by default, though a user could re-enable this
        // option with `-c`. We turn this off since we are are multithreading,
        // so this option would result in non-deterministic output.
        leptess.set_variable(leptess::Variable::ClassifyEnableLearning, "0")?;
        // 7 is PSM_SINGLE_LINE. We have preprocessed the input into individual
        // lines, and telling Tesseract this fact greatly improves accuracy.
        leptess.set_variable(leptess::Variable::TesseditPagesegMode, "7")?;
        // Add user options.
        for (key, value) in config {
            leptess.set_variable(*key, value)?;
        }
        Ok(Self { leptess })
    }

    /// Set the tesseract image to the given image's contents.
    fn set_image(&mut self, image: GrayImage, dpi: i32) -> Result<()> {
        let mut bytes: Cursor<Vec<u8>> = Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(image).write_to(
            &mut bytes,
            image::ImageOutputFormat::Pnm(PnmSubtype::Graymap(SampleEncoding::Binary)),
        )?;
        self.leptess.set_image_from_mem(bytes.get_ref())?;
        self.leptess.set_source_resolution(dpi);
        Ok(())
    }

    /// Get text.
    fn get_text(&mut self) -> Result<String> {
        Ok(self.leptess.get_utf8_text()?)
    }
}
