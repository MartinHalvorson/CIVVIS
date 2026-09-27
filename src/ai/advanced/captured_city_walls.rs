//! Optional preparation of wall research after a major-city capture.

use super::AdvancedAi;

impl AdvancedAi {
    pub fn enable_captured_city_wall_research(&mut self) {
        self.captured_city_wall_research = true;
    }

    pub fn disable_captured_city_wall_research(&mut self) {
        self.captured_city_wall_research = false;
    }
}

#[cfg(test)]
mod tests;
