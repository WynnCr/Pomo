use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
}

pub const SMOOTH: Spring = Spring {
    stiffness: 200.0,
    damping: 28.0,
};

pub const SNAPPY: Spring = Spring {
    stiffness: 340.0,
    damping: 26.0,
};

pub const SWIFT: Spring = Spring {
    stiffness: 480.0,
    damping: 42.0,
};

pub const GENTLE: Spring = Spring {
    stiffness: 130.0,
    damping: 24.0,
};

const EPSILON: f32 = 0.0004;
const SUB_STEP: f32 = 1.0 / 360.0;

#[derive(Clone, Copy)]
struct Motion {
    value: f32,
    velocity: f32,
    target: f32,
    spring: Spring,
}

impl Motion {
    fn settled(&self) -> bool {
        (self.target - self.value).abs() < EPSILON && self.velocity.abs() < EPSILON * 20.0
    }

    fn step(&mut self, dt: f32) {
        if self.settled() {
            self.value = self.target;
            self.velocity = 0.0;
            return;
        }

        let mut remaining = dt;
        while remaining > 0.0 {
            let h = remaining.min(SUB_STEP);
            let acceleration = self.spring.stiffness * (self.target - self.value)
                - self.spring.damping * self.velocity;
            self.velocity += acceleration * h;
            self.value += self.velocity * h;
            remaining -= h;
        }

        if self.settled() {
            self.value = self.target;
            self.velocity = 0.0;
        }
    }
}

#[derive(Default)]
pub struct MotionSet {
    entries: HashMap<&'static str, Motion>,
    reduced: bool,
}

impl MotionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_reduced(&mut self, reduced: bool) {
        self.reduced = reduced;
    }

    pub fn to(&mut self, key: &'static str, spring: Spring, target: f32) -> f32 {
        let entry = self.entries.entry(key).or_insert(Motion {
            value: target,
            velocity: 0.0,
            target,
            spring,
        });
        entry.spring = spring;
        entry.target = target;
        if self.reduced {
            entry.value = target;
            entry.velocity = 0.0;
        }
        entry.value
    }

    pub fn jump(&mut self, key: &'static str, value: f32) {
        let entry = self.entries.entry(key).or_insert(Motion {
            value,
            velocity: 0.0,
            target: value,
            spring: SMOOTH,
        });
        entry.value = value;
        entry.velocity = 0.0;
    }

    pub fn value(&self, key: &'static str) -> f32 {
        self.entries.get(key).map(|m| m.value).unwrap_or(0.0)
    }

    pub fn step(&mut self, dt: f32) -> bool {
        if self.reduced {
            return false;
        }

        let mut animating = false;
        for motion in self.entries.values_mut() {
            motion.step(dt);
            animating |= !motion.settled();
        }
        animating
    }

    pub fn animating(&self) -> bool {
        !self.reduced && self.entries.values().any(|m| !m.settled())
    }
}

/// Cubic ease-out
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}
