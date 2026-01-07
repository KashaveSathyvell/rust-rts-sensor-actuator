#[derive(Debug)]
pub struct PidController {
    kp: f64,
    ki: f64,
    kd: f64,
    integral: f64,
    prev_error: f64,
}

impl PidController {
    pub fn new(kp: f64, ki: f64, kd: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: 0.0,
            prev_error: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
    }

    pub fn compute(&mut self, error: f64, dt: f64) -> f64 {
        // Integral with simple anti-windup
        self.integral += error * dt;
        self.integral = self.integral.clamp(-100.0, 100.0);

        let derivative = if dt > 0.0 {
            (error - self.prev_error) / dt
        } else {
            0.0
        };

        self.prev_error = error;

        (self.kp * error) + (self.ki * self.integral) + (self.kd * derivative)
    }
}
