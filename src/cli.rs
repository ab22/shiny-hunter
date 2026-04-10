use clap::Parser;

use crate::presets::RoiPreset;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    // Camera index from which to capture image output from.
    #[arg(short, long, default_value_t = 1)]
    pub cam_index: i32,

    // Enable debug logs.
    #[arg(short, long)]
    pub debug: bool,

    // Create and display window with the captured output from `cam_index`. Value is set
    // to `true` if --trace is specified.
    #[arg(short, long)]
    pub show: bool,

    // Trace will display a window with the captured input on which you will be able
    // to draw a square for a customized ROI.
    #[arg(long)]
    pub trace: bool,

    // Specifies which ROI preset to use.
    #[arg(long, default_value_t = RoiPreset::PkmnSummary)]
    pub roi_preset: RoiPreset,
}
