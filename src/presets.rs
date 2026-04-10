use clap::ValueEnum;
use opencv::core::Rect;

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum RoiPreset {
    // Focuses on the Switch's Pokemon Summary view where a teal color is set for shinies.
    PkmnSummary,
}

impl RoiPreset {
    pub fn rect(&self) -> Rect {
        let rect = match self {
            RoiPreset::PkmnSummary => Rect::new(815, 250, 890, 325),
        };

        rect
    }
}
