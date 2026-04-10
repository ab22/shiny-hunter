use clap::Parser;

use crate::presets::RoiPreset;

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

    // Trace will display a window with the captured input on which you will be able
    // to draw a square for a customized ROI.
    #[arg(long)]
    pub trace: bool,

    // Specifies which ROI preset to use. Defaults to `PkmnSummary`.
    #[arg(long)]
    pub roi_preset: Option<RoiPreset>,
}
