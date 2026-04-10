use clap::Parser;

use crate::app::{App, Opts};

mod app;
mod cli;
mod presets;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    init_logger(cli.debug);

    let opts = Opts::from(cli);
    let app = App::new(opts);

    app.run()?;

    Ok(())
}

fn init_logger(debug: bool) {
    let mut level = tracing::Level::INFO;
    if debug {
        level = tracing::Level::DEBUG;
    }

    tracing_subscriber::fmt()
        .pretty()
        .with_thread_names(true)
        .with_max_level(level)
        .with_file(true)
        .with_line_number(true)
        .init();
}
