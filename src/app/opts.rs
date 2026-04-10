use crate::{cli, presets::RoiPreset};

pub struct Opts {
    pub debug: bool,
    pub camera_index: i32,
    pub show: bool,
    pub trace: bool,
    pub roi: RoiPreset,
}

impl From<cli::Cli> for Opts {
    fn from(value: cli::Cli) -> Self {
        Self {
            debug: value.debug,
            camera_index: value.cam_index,
            roi: value.roi_preset,
            show: value.show,
            trace: value.trace,
        }
    }
}
