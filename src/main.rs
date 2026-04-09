use std::sync::{Arc, RwLock};

use clap::Parser;
use opencv::{
    core::{self, MatTraitConst, Scalar_},
    highgui, imgproc,
    videoio::{self, VideoCaptureTrait, VideoCaptureTraitConst},
};

mod cli;

fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let cam_index = cli.cam_index.unwrap_or_default();

    println!("OpenCV version: {}", opencv::core::get_version_string()?);
    println!("Capturing from {cam_index} camera index");

    // let pkmn_summary_roi = core::Rect::new();

    draw_debug(cam_index)?;

    Ok(())
}

struct ROISelector {
    start_point: core::Point,
    end_point: core::Point,
    selecting: bool,
    roi: Option<core::Rect>,
}

impl ROISelector {
    fn new() -> Self {
        Self {
            start_point: core::Point::new(0, 0),
            end_point: core::Point::new(0, 0),
            selecting: false,
            roi: None,
        }
    }
}

fn draw_debug(idx: i32) -> anyhow::Result<()> {
    let mut cam = videoio::VideoCapture::new(idx, videoio::CAP_AVFOUNDATION)?;

    if !cam.is_opened()? {
        anyhow::bail!("Camera is not open!");
    }

    let window_name = "Shiny Hunter";
    let roi = core::Rect::new(815, 250, 890, 325);
    let rect_color = core::Scalar::new(0.0, 255.0, 0.0, 0.0);
    let selector = Arc::new(RwLock::new(ROISelector::new()));
    let ui_sel = Arc::clone(&selector);

    highgui::named_window(window_name, highgui::WINDOW_NORMAL)?;
    highgui::set_mouse_callback(
        window_name,
        Some(Box::new(move |event, x, y, _flags| unsafe {
            match event {
                highgui::EVENT_LBUTTONDOWN => {
                    println!("LBUTTON DOWN");
                    let mut s = ui_sel.write().unwrap();
                    s.selecting = true;
                    s.start_point = core::Point::new(x, y);
                    s.end_point = core::Point::new(x, y);
                }
                highgui::EVENT_MOUSEMOVE => {
                    let mut s = ui_sel.write().unwrap();
                    if s.selecting {
                        s.end_point = core::Point::new(x, y);
                    }
                }
                highgui::EVENT_LBUTTONUP => {
                    println!("LBUTTON UP");
                    let mut s = ui_sel.write().unwrap();
                    s.selecting = false;
                    s.roi = Some(core::Rect::new(
                        s.start_point.x.min(x),
                        s.start_point.y.min(y),
                        (s.start_point.x - x).abs(),
                        (s.start_point.y - y).abs(),
                    ));
                }
                _ => {}
            }
        })),
    )?;

    println!("Press 'q' or Esc to quit");

    loop {
        let mut frame = opencv::core::Mat::default();
        if !cam.read(&mut frame)? || frame.empty() {
            anyhow::bail!("No frame to read");
        }

        {
            let s = selector.read().unwrap();

            if s.selecting {
                let rect = core::Rect::new(
                    s.start_point.x.min(s.end_point.x),
                    s.start_point.y.min(s.end_point.y),
                    (s.start_point.x - s.end_point.x).abs(),
                    (s.start_point.y - s.end_point.y).abs(),
                );

                imgproc::rectangle(&mut frame, rect, rect_color, 1, imgproc::LINE_8, 0)?;
            }

            if let Some(rect) = s.roi {
                imgproc::rectangle(&mut frame, rect, rect_color, 3, imgproc::LINE_8, 0)?;
            }
        }

        imgproc::rectangle(&mut frame, roi, rect_color, 3, imgproc::LINE_8, 0)?;
        highgui::imshow(window_name, &frame)?;

        let key = highgui::wait_key(1)?;
        if key == 113 || key == 27 {
            break;
        }
    }

    Ok(())
}
