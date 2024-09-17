//! Application to run OCR on a subtitles image format (like `VobSub`)

use std::{
    io::{self, stdout},
    panic::{set_hook, take_hook},
};

use anyhow::Context;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::LevelFilter;
use ratatui::{
    prelude::{Backend, CrosstermBackend},
    Terminal,
};
use subtile_ocr::{run, Opt};

#[cfg(not(feature = "profile-with-puffin"))]
use no_profiling as prof;
#[cfg(feature = "profile-with-puffin")]
use puffin_profiling as prof;

fn main() -> anyhow::Result<()> {
    init_panic_hook();
    let profiling_data = prof::init();

    simple_logger::SimpleLogger::new()
        .without_timestamps()
        .with_level(LevelFilter::Warn)
        .env()
        .init()
        .unwrap();
    let opt = Opt::parse();
    let tui = init_tui()?;
    let res = run(&opt, tui).with_context(|| {
        format!(
            "Could not convert '{}' to 'srt'.",
            opt.input.clone().display()
        )
    });
    restore_tui()?;

    profiling::finish_frame!();
    prof::write_perf_file(profiling_data)?;

    res
}

fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let _ = restore_tui();
        original_hook(panic_info);
    }));
}
fn init_tui() -> io::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}
fn restore_tui() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(not(feature = "profile-with-puffin"))]
mod no_profiling {
    pub struct Empty;
    pub fn init() -> Empty {
        Empty {}
    }
    pub fn write_perf_file(_: Empty) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "profile-with-puffin")]
mod puffin_profiling {
    use chrono::Local;
    use profiling::puffin::{self, GlobalFrameView};
    use std::fs::{self, File};

    pub fn init() -> GlobalFrameView {
        let global_frame_view = GlobalFrameView::default();
        puffin::set_scopes_on(true);
        global_frame_view
    }

    pub fn write_perf_file(global_frame_view: GlobalFrameView) -> anyhow::Result<()> {
        let now = Local::now().format("%Y-%m-%d-%T").to_string();
        let filename = format!("perf/capture_{now}.puffin");

        fs::create_dir_all("perf")?;
        let mut file = File::create(filename)?;
        let frame_view = global_frame_view.lock();
        (*frame_view).write(&mut file)?;
        Ok(())
    }
}
