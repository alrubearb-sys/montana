// Size of the box the chart paths are drawn in, with y growing downwards. The
// panels stretch it to the size they draw the charts at.
pub const WIDTH: f64 = 370.0;
pub const HEIGHT: f64 = 68.0;
const STEPS: [f64; 8] = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0];
const MAX_STEPS: f64 = 4.0;
// A degree is as tall in every place, so that a warm day and a cold one do not
// look alike.
const TEMPERATURE_SPAN: f64 = 20.0;
const TEMPERATURE_STEP: f64 = 5.0;
// Space on each side of a bar, as a part of its hour, so that bars never touch.
const BAR_GAP: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    low: f64,
    high: f64,
    step: f64,
}

impl Scale {
    /// `TEMPERATURE_SPAN` degrees around the values, wider only when they need
    /// it, and always with 0 to show on which side of freezing they are.
    pub fn temperature(values: &[f64]) -> Self {
        let low = values.iter().copied().fold(f64::INFINITY, f64::min);
        let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if !low.is_finite() || !high.is_finite() {
            return Self::between(-TEMPERATURE_SPAN / 2.0, TEMPERATURE_SPAN / 2.0);
        }
        let middle = f64::midpoint(low, high);
        let bottom =
            ((middle - TEMPERATURE_SPAN / 2.0) / TEMPERATURE_STEP).floor() * TEMPERATURE_STEP;
        let top = bottom + TEMPERATURE_SPAN;
        Self::between(bottom.min(low).min(0.0), top.max(high).max(0.0))
    }

    pub fn from_zero(values: &[f64], at_least: f64) -> Self {
        Self::between(0.0, values.iter().copied().fold(at_least, f64::max))
    }

    fn between(low: f64, high: f64) -> Self {
        let fit = |step: f64| {
            let low = (low / step).floor() * step;
            let high = ((high / step).ceil() * step).max(low + step);
            Self { low, high, step }
        };
        STEPS
            .iter()
            .map(|step| fit(*step))
            .find(|scale| (scale.high - scale.low) / scale.step <= MAX_STEPS)
            .unwrap_or_else(|| fit(500.0))
    }

    /// 0 at the top of the chart and 1 at the bottom.
    pub fn position(self, value: f64) -> f64 {
        (self.high - value) / (self.high - self.low)
    }

    pub fn ticks(self) -> Vec<f64> {
        let mut ticks = Vec::new();
        let mut value = self.low;
        while value <= self.high + self.step / 2.0 {
            ticks.push(value);
            value += self.step;
        }
        ticks
    }
}

pub fn line(values: &[f64], scale: Scale) -> String {
    let slot = slot(values);
    let mut path = String::new();
    for (index, value) in values.iter().enumerate() {
        let command = if index == 0 { 'M' } else { 'L' };
        let x = (position(index) + 0.5) * slot;
        let y = scale.position(*value) * HEIGHT;
        path.push_str(&format!("{command} {x:.1} {y:.1} "));
    }
    path
}

pub fn bars(values: &[f64], scale: Scale) -> String {
    let slot = slot(values);
    let mut path = String::new();
    for (index, value) in values.iter().enumerate().filter(|(_, value)| **value > 0.0) {
        let left = (position(index) + BAR_GAP) * slot;
        let right = (position(index) + 1.0 - BAR_GAP) * slot;
        let top = scale.position(*value) * HEIGHT;
        path.push_str(&format!(
            "M {left:.1} {HEIGHT} L {left:.1} {top:.1} L {right:.1} {top:.1} L {right:.1} {HEIGHT} Z "
        ));
    }
    path
}

fn slot(values: &[f64]) -> f64 {
    WIDTH / position(values.len().max(1))
}

fn position(index: usize) -> f64 {
    u32::try_from(index).map_or(0.0, f64::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_has_the_same_span_everywhere() {
        let st_gallen = Scale::temperature(&[0.2, 2.1]);
        let spain = Scale::temperature(&[9.0, 11.4]);

        assert_eq!(st_gallen.ticks(), [-10.0, -5.0, 0.0, 5.0, 10.0]);
        assert_eq!(spain.ticks(), [0.0, 5.0, 10.0, 15.0, 20.0]);
    }

    #[test]
    fn temperature_keeps_zero_and_widens_for_big_swings() {
        assert_eq!(
            Scale::temperature(&[24.0, 27.0]).ticks(),
            [0.0, 10.0, 20.0, 30.0, 40.0]
        );
        assert_eq!(
            Scale::temperature(&[-14.0, 9.0]).ticks(),
            [-20.0, -10.0, 0.0, 10.0]
        );
        assert_eq!(
            Scale::temperature(&[]).ticks(),
            [-10.0, -5.0, 0.0, 5.0, 10.0]
        );
    }

    #[test]
    fn scale_has_round_ticks() {
        assert_eq!(Scale::from_zero(&[0.0, 0.3], 1.0).ticks(), [0.0, 1.0]);
        assert_eq!(
            Scale::from_zero(&[73.0], 1.0).ticks(),
            [0.0, 20.0, 40.0, 60.0, 80.0]
        );
    }

    #[test]
    fn high_values_are_at_the_top() {
        let scale = Scale::from_zero(&[80.0], 1.0);

        assert_eq!(scale.position(80.0), 0.0);
        assert_eq!(scale.position(0.0), 1.0);
    }

    #[test]
    fn line_goes_through_the_middle_of_each_hour() {
        let scale = Scale::from_zero(&[4.0], 1.0);

        assert_eq!(line(&[0.0, 4.0], scale), "M 92.5 68.0 L 277.5 0.0 ");
    }

    #[test]
    fn bars_skip_empty_hours() {
        let scale = Scale::from_zero(&[4.0], 1.0);

        assert_eq!(bars(&[0.0, 0.0], scale), "");
        assert_eq!(
            bars(&[0.0, 2.0], scale),
            "M 203.5 68 L 203.5 34.0 L 351.5 34.0 L 351.5 68 Z "
        );
    }
}
