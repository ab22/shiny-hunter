use opencv::videoio::{self, VideoCaptureTraitConst};

fn main() -> anyhow::Result<()> {
    const BACKENDS: [i32; 2] = [videoio::CAP_ANY, videoio::CAP_AVFOUNDATION];

    for i in 0..10 {
        for &backend in &BACKENDS {
            let cam = if let Ok(cam) = videoio::VideoCapture::new(i, backend) {
                cam
            } else {
                continue;
            };
            let is_opened = if let Ok(is_opened) = cam.is_opened() {
                is_opened
            } else {
                continue;
            };

            if !is_opened {
                continue;
            }

            let width = cam.get(videoio::CAP_PROP_FRAME_WIDTH).unwrap_or(0.0);
            let height = cam.get(videoio::CAP_PROP_FRAME_HEIGHT).unwrap_or(0.0);
            let backend_name = match backend {
                videoio::CAP_ANY => "Default",
                videoio::CAP_AVFOUNDATION => "AVFoundation",
                _ => "Unknown",
            };

            println!("[{i}] Camera on backend {}", backend_name);
            println!("  Resolution: {}x{}", width, height);
            break;
        }
    }

    Ok(())
}
