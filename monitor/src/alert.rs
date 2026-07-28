pub const REARM_MARGIN_C: f64 = 5.0;
pub const RATE_LIMIT_SECS: f64 = 60.0;

/// Tracks per-alert hysteresis + rate limiting.
#[derive(Debug)]
pub struct Category {
    pub threshold_c: f64,
    pub armed: bool,
    pub last_fired: f64,
}

impl Category {
    pub fn new(threshold_c: f64) -> Self {
        Self {
            threshold_c,
            armed: true,
            last_fired: -RATE_LIMIT_SECS,
        }
    }

    pub fn should_fire(&mut self, driving_temp_c: Option<f64>, now: f64) -> bool {
        let temp = match driving_temp_c {
            Some(t) => t,
            None => return false,
        };

        // Re-arm once temperature falls well below threshold
        if !self.armed && temp <= self.threshold_c - REARM_MARGIN_C {
            self.armed = true;
        }

        if !self.armed {
            return false;
        }

        if temp < self.threshold_c {
            return false;
        }

        if now - self.last_fired < RATE_LIMIT_SECS {
            return false;
        }

        self.armed = false;
        self.last_fired = now;
        true
    }
}

/// Fires when temperature stays above threshold continuously for `required_secs`.
#[derive(Debug)]
pub struct SustainedAlert {
    pub threshold_c: f64,
    pub required_secs: f64,
    pub above_since: Option<f64>,
    pub last_fired: f64,
    pub armed: bool,
}

impl SustainedAlert {
    pub fn new(threshold_c: f64, required_secs: f64) -> Self {
        Self {
            threshold_c,
            required_secs,
            above_since: None,
            last_fired: -RATE_LIMIT_SECS,
            armed: true,
        }
    }

    pub fn update(&mut self, temp_c: Option<f64>, now: f64) -> bool {
        let temp = match temp_c {
            Some(t) => t,
            None => {
                self.above_since = None;
                return false;
            }
        };

        if !self.armed {
            if temp <= self.threshold_c - REARM_MARGIN_C {
                self.armed = true;
            }
            return false;
        }

        if temp >= self.threshold_c {
            if self.above_since.is_none() {
                self.above_since = Some(now);
            }

            let elapsed = now - self.above_since.unwrap_or(now);
            if elapsed >= self.required_secs && (now - self.last_fired >= RATE_LIMIT_SECS) {
                self.last_fired = now;
                self.above_since = None;
                self.armed = false;
                return true;
            }
        } else if temp <= self.threshold_c - REARM_MARGIN_C {
            self.above_since = None;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_trigger_and_rearm() {
        let mut cat = Category::new(83.0);
        let mut time = 100.0;

        // Below threshold - should not fire
        assert!(!cat.should_fire(Some(80.0), time));

        // At threshold - should fire
        assert!(cat.should_fire(Some(83.0), time));

        // Immediately after - disarmed & rate-limited
        assert!(!cat.should_fire(Some(88.0), time + 5.0));

        // Time passes (70s), but temp is still above threshold minus margin (80°C > 78°C)
        // so it hasn't re-armed yet
        time += 70.0;
        assert!(!cat.should_fire(Some(80.0), time));

        // Temp drops to 77°C (below 83 - 5 = 78°C) -> re-arms
        assert!(!cat.should_fire(Some(77.0), time));

        // Now temp spikes again -> should fire
        time += 1.0;
        assert!(cat.should_fire(Some(85.0), time));
    }

    #[test]
    fn test_category_rate_limit() {
        let mut cat = Category::new(80.0);
        let now = 100.0;

        assert!(cat.should_fire(Some(85.0), now));

        // Cool down to re-arm immediately
        cat.should_fire(Some(70.0), now + 1.0);

        // Try to fire before rate limit expires (now + 10s < 100 + 60s)
        assert!(!cat.should_fire(Some(85.0), now + 10.0));

        // Try after rate limit expires (now + 65s > 100 + 60s)
        assert!(cat.should_fire(Some(85.0), now + 65.0));
    }

    #[test]
    fn test_sustained_alert_timer() {
        let mut alert = SustainedAlert::new(85.0, 10.0);
        let mut time = 0.0;

        // Below threshold - no timer
        assert!(!alert.update(Some(80.0), time));
        assert!(alert.above_since.is_none());

        // Above threshold starts timer
        assert!(!alert.update(Some(86.0), time));
        assert_eq!(alert.above_since, Some(0.0));

        // 5 seconds elapsed - not reached 10s yet
        time = 5.0;
        assert!(!alert.update(Some(87.0), time));

        // Temp drops below margin -> resets timer
        time = 6.0;
        assert!(!alert.update(Some(79.0), time));
        assert!(alert.above_since.is_none());

        // Above threshold again at 10s
        time = 10.0;
        assert!(!alert.update(Some(88.0), time));

        // Holds above threshold for 10s -> fires at 20s (elapsed = 10.0)
        time = 20.0;
        assert!(alert.update(Some(88.0), time));

        // Disarmed until cooled down
        time = 21.0;
        assert!(!alert.update(Some(88.0), time));
    }

    #[test]
    fn test_sustained_alert_none_temp_resets_timer() {
        let mut alert = SustainedAlert::new(85.0, 10.0);
        let mut time = 0.0;

        assert!(!alert.update(Some(86.0), time));
        assert!(alert.above_since.is_some());

        time = 5.0;
        assert!(!alert.update(None, time));
        assert!(alert.above_since.is_none());
    }

    #[test]
    fn test_category_none_temp_handling() {
        let mut cat = Category::new(80.0);
        assert!(!cat.should_fire(None, 100.0));
    }
}
