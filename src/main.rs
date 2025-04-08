//! Application to run OCR on a subtitles image format (like `VobSub`)

use anyhow::Context;
use clap::Parser;
use log::LevelFilter;
use subtile_ocr::{run, Opt};

#[cfg(not(feature = "profile-with-puffin"))]
use no_profiling as prof;
#[cfg(feature = "profile-with-puffin")]
use puffin_profiling as prof;

fn main() -> anyhow::Result<()> {
    let profiling_data = prof::init();

    simple_logger::SimpleLogger::new()
        .without_timestamps()
        .with_level(LevelFilter::Warn)
        .env()
        .init()
        .unwrap();
    let opt = Opt::parse();
    let res = run(&opt).with_context(|| {
        format!(
            "Could not convert '{}' to 'srt'.",
            opt.input.clone().display()
        )
    });

    profiling::finish_frame!();
    prof::write_perf_file(profiling_data)?;

    res
}

#[cfg(not(feature = "profile-with-puffin"))]
mod no_profiling {
    pub struct Empty;
    pub const fn init() -> Empty {
        Empty {}
    }
    pub const fn write_perf_file(_: Empty) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "profile-with-puffin")]
mod puffin_profiling {
    use chrono::Local;
    use profiling::puffin::{self, GlobalFrameView};
    use std::{
        fs::{self, File},
        io::BufWriter,
    };

    pub fn init() -> GlobalFrameView {
        let global_frame_view = GlobalFrameView::default();
        puffin::set_scopes_on(true);
        global_frame_view
    }

    pub fn write_perf_file(global_frame_view: GlobalFrameView) -> anyhow::Result<()> {
        let now = Local::now().format("%Y-%m-%d-%T").to_string();
        let filename = format!("perf/capture_{now}.puffin");

        fs::create_dir_all("perf")?;
        let mut file = BufWriter::new(File::create(filename)?);
        let frame_view = global_frame_view.lock();
        (*frame_view).write(&mut file)?;
        Ok(())
    }
}
