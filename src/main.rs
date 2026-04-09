use clap::Parser;

mod cli;

fn main() -> Result<(), opencv::Error> {
    let cli = cli::Cli::parse();
    let cam_index = cli.cam_index.unwrap_or_default();

    println!("OpenCV version: {}", opencv::core::get_version_string()?);
    println!("Capturing from {cam_index} camera index");

    Ok(())
}
