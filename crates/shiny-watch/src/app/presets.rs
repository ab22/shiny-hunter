use clap::ValueEnum;
use opencv::core::{Point, Rect, Scalar};
use std::fmt::Display;

pub struct RegionOfInterest {
    pub point: Point,
    pub target_color: Scalar,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum RoiPreset {
    // Focuses on the Switch's Pokemon Summary view where a teal color is set for shinies.
    PkmnSummaryView,
    // Sample Fire Red Charmander image on OBS
    TestImage,
}

impl RoiPreset {
    pub fn get(&self) -> RegionOfInterest {
        match self {
            RoiPreset::PkmnSummaryView => RegionOfInterest {
                point: Point::new(265, 109),
                target_color: Scalar::new(231.0, 231.0, 107.0, 0.0),
            },

            RoiPreset::TestImage => RegionOfInterest {
                point: Point::new(262, 111),
                target_color: Scalar::new(229.0, 244.0, 119.0, 0.0),
            },
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
