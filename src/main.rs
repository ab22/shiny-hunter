use clap::Parser;

use crate::app::{App, Opts};

mod app;
mod cli;
mod presets;

fn main() -> anyhow::Result<()> {
    init_logger();

    let cli = cli::Cli::parse();
    let opts = Opts::from(cli);
    let app = App::new(opts);

    app.run()?;

    Ok(())
}

fn init_logger() {
    tracing_subscriber::fmt()
        .pretty()
        .with_thread_names(true)
        .with_max_level(tracing::Level::INFO)
        .with_file(true)
        .with_line_number(true)
        .init();
}
