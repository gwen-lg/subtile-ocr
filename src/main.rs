use anyhow::Context;
use clap::Parser;
use log::LevelFilter;
use subtile_ocr::{run, Opt};

fn main() -> anyhow::Result<()> {
    simple_logger::SimpleLogger::new()
        .without_timestamps()
        .with_level(LevelFilter::Warn)
        .env()
        .init()
        .unwrap();
    let opt = Opt::parse();
    run(&opt).with_context(|| {
        format!(
            "Could not convert '{}' to 'srt'.",
            opt.input.clone().display()
        )
    })
}
