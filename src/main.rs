//! Application to run OCR on a subtitles image format (like `VobSub`)

use anyhow::Context;
use chrono::Local;
use clap::Parser;
use log::LevelFilter;
use subtile_ocr::{run, Opt};

#[cfg(not(feature = "profile-with-puffin"))]
use no_profiling as prof;
#[cfg(feature = "profile-with-puffin")]
use puffin_profiling as prof;

use alloc_track::{AllocTrack, BacktraceMetric, BacktraceMode};
use std::{alloc::System, fs::File, io::Write};

#[global_allocator]
static GLOBAL_ALLOC: AllocTrack<System> = AllocTrack::new(System, BacktraceMode::Short);

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
    prof::write_perf_file(&profiling_data)?;

    mem_stats_report()?;

    res
}

fn mem_stats_report() -> Result<(), anyhow::Error> {
    let now = Local::now().format("%Y-%m-%d-%T").to_string();
    let filename = format!("perf/alloc_backtrace_{now}.txt");
    let mut file = File::create(filename)?;

    // Summary
    let backtrace_report = alloc_track::backtrace_report(|_, _| true);
    let summary =
        backtrace_report
            .0
            .iter()
            .fold(BacktraceMetric::default(), |val, (_, cur_metric)| {
                BacktraceMetric {
                    allocated: val.allocated + cur_metric.allocated,
                    freed: val.freed + cur_metric.freed,
                    allocations: val.allocations + cur_metric.allocations,
                    mode: BacktraceMode::None,
                }
            });
    writeln!(&mut file, "Summary : \n{summary}")?;

    // with Filter and Sort
    let mut backtrace_report = alloc_track::backtrace_report(|_, metrics| metrics.allocations > 10);
    backtrace_report.0.sort_unstable_by(|(_, a), (_, b)| {
        a.allocations.partial_cmp(&b.allocations).unwrap().reverse()
    });
    writeln!(&file, "Details : \n{backtrace_report}")?;

    Ok(())
}

#[cfg(not(feature = "profile-with-puffin"))]
mod no_profiling {
    pub struct Empty;
    pub const fn init() -> Empty {
        Empty {}
    }
    pub const fn write_perf_file(_: &Empty) -> anyhow::Result<()> {
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

    pub fn write_perf_file(global_frame_view: &GlobalFrameView) -> anyhow::Result<()> {
        let now = Local::now().format("%Y-%m-%d-%T").to_string();
        let filename = format!("perf/capture_{now}.puffin");

        fs::create_dir_all("perf")?;
        let mut file = BufWriter::new(File::create(filename)?);
        let frame_view = global_frame_view.lock();
        (*frame_view).write(&mut file)?;
        Ok(())
    }
}
