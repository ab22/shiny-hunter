use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    // Camera index from which to capture image output from.
    #[arg(short, long)]
    pub cam_index: Option<i32>,

    // Enable debug mode
    #[arg(short, long)]
    pub debug: bool,

    // Show Region of Interest window
    #[arg(short, long)]
    pub show: bool,
}
