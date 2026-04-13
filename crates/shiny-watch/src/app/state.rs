use opencv::core::Point;

pub struct State {
    pub clicked_point: Option<Point>,
}

impl State {
    pub fn new() -> Self {
        Self {
            clicked_point: None,
        }
    }
}
