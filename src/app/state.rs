use opencv::core::{Point, Rect};

pub struct State {
    pub selected_roi: Option<Rect>,
    pub selecting: bool,
    pub start_point: Point,
    pub end_point: Point,
}

impl State {
    pub fn new() -> Self {
        Self {
            selected_roi: None,
            selecting: false,
            start_point: Point::default(),
            end_point: Point::default(),
        }
    }
}
