use std::fmt::Display;

use clap::ValueEnum;
use opencv::core::{Rect, Scalar};

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum RoiPreset {
    // Focuses on the Switch's Pokemon Summary view where a teal color is set for shinies.
    PkmnSummaryView,
    // Sample Fire Red Charmander image on OBS
    TestImage,
}

impl RoiPreset {
    pub fn rect(&self) -> Rect {
        let rect = match self {
            RoiPreset::PkmnSummaryView => Rect::new(262, 111, 23, 15),
            RoiPreset::TestImage => Rect::new(262, 111, 23, 15),
        };

        rect
    }

    pub fn target_color(&self) -> Scalar {
        match self {
            RoiPreset::PkmnSummaryView => Scalar::new(231.0, 231.0, 107.0, 0.0),
            RoiPreset::TestImage => Scalar::new(229.0, 244.0, 119.0, 0.0),
        }
    }
}

impl Display for RoiPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PkmnSummaryView => write!(f, "pkmn-summary-view"),
            Self::TestImage => write!(f, "test-image"),
        }
    }
}
