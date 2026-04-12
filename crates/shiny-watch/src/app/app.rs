use std::sync::{Arc, RwLock};

use anyhow::Result;
use opencv::{
    core::{Mat, MatTraitConst, Point, Rect, Scalar, ToInputOutputArray},
    highgui, imgproc,
    videoio::{self, VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};
use tracing::info;

use crate::app::{Opts, State, color};

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

        let roi_preset = self.opts.roi.rect();
        let target_color = self.opts.roi.target_color();

        loop {
            let mut frame = Mat::default();
            if !cam.read(&mut frame)? {
                anyhow::bail!("Failed to grab frame");
            } else if frame.empty() {
                anyhow::bail!("Frame was empty!");
            }

            if self.roi_has_color(&mut frame, roi_preset, target_color)? {
                info!("SHINY ENCOUNTER!");
            }

            // Draw red rectangles after all checks to avoid issues.
            imgproc::rectangle(&mut frame, roi_preset, color::RED, 3, imgproc::LINE_8, 0)?;
            if self.opts.trace {
                self.draw_trace_rect(&mut frame)?;
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

    fn draw_trace_rect(&self, frame: &mut impl ToInputOutputArray) -> Result<()> {
        let mut selecting_rect: Option<Rect> = None;
        let mut selected_roi: Option<Rect> = None;

        {
            let s = self.state.read().unwrap();

            if s.selecting {
                selecting_rect = Some(Rect::new(
                    s.start_point.x.min(s.end_point.x),
                    s.start_point.y.min(s.end_point.y),
                    (s.start_point.x - s.end_point.x).abs(),
                    (s.start_point.y - s.end_point.y).abs(),
                ));
            }

            if let Some(r) = s.selected_roi {
                selected_roi = Some(r.clone());
            }
        }

        if let Some(r) = selecting_rect {
            imgproc::rectangle(frame, r, color::RED, 1, imgproc::LINE_8, 0)?;
        }

        if let Some(r) = selected_roi {
            imgproc::rectangle(frame, r, color::RED, 3, imgproc::LINE_8, 0)?;
        }

        Ok(())
    }

    fn hook_mouse_callback(&self, wnd_name: &str) -> Result<()> {
        let state = Arc::clone(&self.state);

        highgui::set_mouse_callback(
            wnd_name,
            Some(Box::new(move |event, x, y, _flags| match event {
                highgui::EVENT_LBUTTONDOWN => {
                    let mut s = state.write().unwrap();
                    s.selecting = true;
                    s.start_point = Point::new(x, y);
                    s.end_point = Point::new(x, y);
                }
                highgui::EVENT_MOUSEMOVE => {
                    let mut s = state.write().unwrap();
                    if s.selecting {
                        s.end_point = Point::new(x, y);
                    }
                }
                highgui::EVENT_LBUTTONUP => {
                    let mut s = state.write().unwrap();
                    let r = Rect::new(
                        s.start_point.x.min(x),
                        s.start_point.y.min(y),
                        (s.start_point.x - x).abs(),
                        (s.start_point.y - y).abs(),
                    );

                    info!(
                        x1 = r.x,
                        y2 = r.y,
                        x2 = r.width,
                        y2 = r.height,
                        "New Selected Rectangle"
                    );
                    s.selecting = false;
                    s.selected_roi = Some(r);
                }
                _ => {}
            })),
        )?;

        Ok(())
    }

    fn roi_has_color(
        &self,
        frame: &impl MatTraitConst,
        roi: Rect,
        target_color: Scalar,
    ) -> Result<bool> {
        let roi_image = Mat::roi(frame, roi)?;
        let tolerance = 1.0;
        let lower_bound = Scalar::new(
            (target_color[0] - tolerance).max(0.0),
            (target_color[1] - tolerance).max(0.0),
            (target_color[2] - tolerance).max(0.0),
            0.0,
        );
        let upper_bound = Scalar::new(
            (target_color[0] - tolerance).max(255.0),
            (target_color[1] - tolerance).max(255.0),
            (target_color[2] - tolerance).max(255.0),
            0.0,
        );

        let mut mask = Mat::default();
        opencv::core::in_range(&roi_image, &lower_bound, &upper_bound, &mut mask)?;

        let matching_pixels = opencv::core::count_non_zero(&mask)?;

        if self.opts.debug {
            let mut average_color = Mat::default();
            let mut roi_f32 = Mat::default();
            roi_image.convert_to(&mut roi_f32, opencv::core::CV_32FC3, 1.0, 0.0)?;
            opencv::core::reduce(
                &roi_f32,
                &mut average_color,
                0,
                opencv::core::REDUCE_AVG,
                -1,
            )?;

            let avg_pixel = average_color.at_2d::<opencv::core::Vec3f>(0, 0)?;
            let avg_b = avg_pixel[0];
            let avg_g = avg_pixel[1];
            let avg_r = avg_pixel[2];
            info!(
                "{}",
                format!(
                    "ROI Average Color: B: {:.1}, G: {:.1}, R: {:.1}",
                    avg_b, avg_g, avg_r,
                )
            );
        }

        Ok(matching_pixels > 0)
    }
}
