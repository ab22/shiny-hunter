use std::sync::{Arc, RwLock};

use anyhow::Result;
use opencv::{
    core::{Mat, MatTraitConst, Point, ToInputOutputArray, Vec3b},
    highgui, imgproc,
    videoio::{self, VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};
use tracing::info;

use crate::app::{Opts, State, color, presets::RegionOfInterest};

pub struct App {
    opts: Opts,
    state: Arc<RwLock<State>>,
}

impl App {
    pub fn new(opts: Opts) -> Self {
        let state = Arc::new(RwLock::new(State::new()));

        Self { opts, state }
    }

    pub fn run(&self) -> Result<()> {
        let opencv_version = opencv::core::get_version_string()?;

        info!(
            opencv_version,
            camera_index = self.opts.camera_index,
            "Staring Shiny Hunter"
        );

        let mut cam = self.get_cam()?;
        let wnd_name = "ShinyHunter";

        self.setup(wnd_name)?;
        self.draw(wnd_name, &mut cam)?;

        Ok(())
    }

    fn get_cam(&self) -> Result<videoio::VideoCapture> {
        let cam = videoio::VideoCapture::new(self.opts.camera_index, videoio::CAP_AVFOUNDATION)?;
        if !cam.is_opened()? {
            anyhow::bail!("Camera is not open!");
        }

        Ok(cam)
    }

    fn setup(&self, wnd_name: &str) -> Result<()> {
        highgui::named_window(wnd_name, highgui::WINDOW_NORMAL)?;

        if self.opts.trace {
            self.hook_mouse_callback(wnd_name)?;
        }

        Ok(())
    }

    fn draw(&self, wnd_name: &str, cam: &mut VideoCapture) -> Result<()> {
        println!("Press 'q' or Esc to quit");

        let roi = self.opts.roi.get();

        loop {
            let mut frame = Mat::default();
            if !cam.read(&mut frame)? {
                anyhow::bail!("Failed to grab frame");
            } else if frame.empty() {
                anyhow::bail!("Frame was empty!");
            }

            if self.roi_has_color(&mut frame, &roi)? {
                info!("SHINY ENCOUNTER!");
            }

            self.draw_crosshair(&mut frame, roi.point)?;
            if self.opts.trace {
                self.draw_clicked_point(&mut frame)?;
            }

            if self.opts.show {
                highgui::imshow(wnd_name, &frame)?;
            }

            let key = highgui::wait_key(1)?;
            if key == 113 || key == 27 {
                break;
            }
        }

        Ok(())
    }

    fn draw_clicked_point(&self, frame: &mut impl ToInputOutputArray) -> Result<()> {
        let mut point: Option<Point> = None;

        {
            let s = self.state.read().unwrap();
            if let Some(clicked) = s.clicked_point {
                point = Some(clicked.clone());
            }
        }

        if let Some(p) = point {
            self.draw_crosshair(frame, p)?;
        }

        Ok(())
    }

    fn draw_crosshair(&self, frame: &mut impl ToInputOutputArray, p: Point) -> Result<()> {
        let size = 10;

        // Horizontal line
        imgproc::line(
            frame,
            Point::new(p.x - size, p.y),
            Point::new(p.x + size, p.y),
            color::RED,
            2,
            imgproc::LINE_8,
            0,
        )?;

        // Vertical line
        imgproc::line(
            frame,
            Point::new(p.x, p.y - size),
            Point::new(p.x, p.y + size),
            color::RED,
            2,
            imgproc::LINE_8,
            0,
        )?;

        Ok(())
    }

    fn hook_mouse_callback(&self, wnd_name: &str) -> Result<()> {
        let state = Arc::clone(&self.state);

        highgui::set_mouse_callback(
            wnd_name,
            Some(Box::new(move |event, x, y, _flags| match event {
                highgui::EVENT_LBUTTONDOWN => {
                    let mut s = state.write().unwrap();
                    s.clicked_point = Some(Point::new(x, y));

                    info!(x, y, "New point selected");
                }
                _ => {}
            })),
        )?;

        Ok(())
    }

    fn roi_has_color(&self, frame: &impl MatTraitConst, roi: &RegionOfInterest) -> Result<bool> {
        let pixel = frame.at_2d::<Vec3b>(roi.point.y, roi.point.x)?;
        let target_color = roi.target_color;
        let target_b = target_color[0] as u8;
        let target_g = target_color[1] as u8;
        let target_r = target_color[2] as u8;
        let tolerance = 20.0 as u8;

        let b_diff = if pixel[0] > target_b {
            pixel[0] - target_b
        } else {
            target_b - pixel[0]
        };
        let g_diff = if pixel[1] > target_g {
            pixel[1] - target_g
        } else {
            target_g - pixel[1]
        };
        let r_diff = if pixel[2] > target_r {
            pixel[2] - target_r
        } else {
            target_r - pixel[2]
        };

        // if self.opts.debug {
        //     let mut average_color = Mat::default();
        //     let mut roi_f32 = Mat::default();
        //     roi_image.convert_to(&mut roi_f32, opencv::core::CV_32FC3, 1.0, 0.0)?;
        //     opencv::core::reduce(
        //         &roi_f32,
        //         &mut average_color,
        //         0,
        //         opencv::core::REDUCE_AVG,
        //         -1,
        //     )?;

        //     let avg_pixel = average_color.at_2d::<opencv::core::Vec3f>(0, 0)?;
        //     let avg_b = avg_pixel[0];
        //     let avg_g = avg_pixel[1];
        //     let avg_r = avg_pixel[2];
        //     info!(
        //         "{}",
        //         format!(
        //             "ROI Average Color: B: {:.1}, G: {:.1}, R: {:.1}",
        //             avg_b, avg_g, avg_r,
        //         )
        //     );
        // }

        Ok(b_diff <= tolerance && g_diff <= tolerance && r_diff <= tolerance)
    }
}
