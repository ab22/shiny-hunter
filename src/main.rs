use clap::Parser;
use opencv::{
    core::MatTraitConst,
    highgui,
    videoio::{self, VideoCaptureTrait, VideoCaptureTraitConst},
};

mod cli;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let cam_index = cli.cam_index.unwrap_or_default();

    println!("OpenCV version: {}", opencv::core::get_version_string()?);
    println!("Capturing from {cam_index} camera index");

    draw_debug(cam_index)?;

    Ok(())
}

fn draw_debug(idx: i32) -> anyhow::Result<()> {
    let mut cam = videoio::VideoCapture::new(idx, videoio::CAP_AVFOUNDATION)?;

    if !cam.is_opened()? {
        println!("Camera is not open");
        return anyhow::bail!("Camera is not open!");
    }

    loop {
        let mut frame = opencv::core::Mat::default();
        if !cam.read(&mut frame)? || frame.empty() {
            println!("No frame to read");
            continue;
        }

        highgui::imshow("Switch", &frame)?;

        println!("Press 'q' or Esc to close");
        let key = highgui::wait_key(1)?;
        if key == 113 || key == 27 {
            break;
        }
    }

    Ok(())
}
