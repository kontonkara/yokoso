//! Motion: springs, and the settings they take their character from.
//!
//! A spring is solved exactly over each step, not integrated, so it moves the
//! same at 60 Hz and at 240, and no stiffness makes it blow up. Aimed
//! somewhere new mid-way, it keeps its speed: interrupting never jerks.

use std::collections::HashMap;

/// How a spring moves: its stiffness, and its damping as a ratio, where 1
/// is the fastest without any bounce and less bounces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringParams {
    pub stiffness: f32,
    pub damping: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Lively, with a bounce: the default.
    Expressive,
    Bouncy,
    Snappy,
    /// No bounce at all.
    Calm,
    Gentle,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Expressive,
        Preset::Bouncy,
        Preset::Snappy,
        Preset::Calm,
        Preset::Gentle,
    ];

    pub fn params(self) -> SpringParams {
        let (stiffness, damping) = match self {
            Preset::Expressive => (380.0, 0.68),
            Preset::Bouncy => (260.0, 0.5),
            Preset::Snappy => (900.0, 0.75),
            Preset::Calm => (545.0, 1.0),
            Preset::Gentle => (170.0, 0.9),
        };
        SpringParams { stiffness, damping }
    }

    pub fn name(self) -> &'static str {
        match self {
            Preset::Expressive => "expressive",
            Preset::Bouncy => "bouncy",
            Preset::Snappy => "snappy",
            Preset::Calm => "calm",
            Preset::Gentle => "gentle",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    /// No bounces anywhere.
    Reduced,
    /// Everything at once.
    Off,
}

/// The parts of the shell that have motion of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Panels,
    Launcher,
    Toasts,
    Osd,
    /// Rows, pages and the like inside panels.
    Content,
    /// Selection and scrolling in lists.
    Lists,
    /// Hover, press, toggles, rolling digits.
    Micro,
}

/// A part's spring: the global one, a preset, or one of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    Global,
    Preset(Preset),
    Custom(SpringParams),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartMotion {
    pub spring: Choice,
    pub speed: f32,
}

/// All of the shell's motion settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    pub global: SpringParams,
    pub speed: f32,
    pub mode: Mode,
    pub parts: HashMap<Part, PartMotion>,
}

impl Default for Motion {
    /// The values chosen live: Expressive at ×1.15, panel content,
    /// lists and small things Snappy.
    fn default() -> Self {
        let snappy = PartMotion {
            spring: Choice::Preset(Preset::Snappy),
            speed: 1.0,
        };
        Self {
            global: Preset::Expressive.params(),
            speed: 1.15,
            mode: Mode::Normal,
            parts: HashMap::from([(Part::Content, snappy), (Part::Lists, snappy), (Part::Micro, snappy)]),
        }
    }
}

/// What a spring of a part steps with: its params and its speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feel {
    pub params: SpringParams,
    pub speed: f32,
    pub mode: Mode,
}

impl Motion {
    pub fn feel(&self, part: Part) -> Feel {
        let own = self.parts.get(&part);
        let mut params = match own.map(|own| own.spring) {
            None | Some(Choice::Global) => self.global,
            Some(Choice::Preset(preset)) => preset.params(),
            Some(Choice::Custom(params)) => params,
        };
        if self.mode == Mode::Reduced {
            params.damping = params.damping.max(1.0);
        }
        Feel {
            params,
            speed: self.speed * own.map_or(1.0, |own| own.speed),
            mode: self.mode,
        }
    }
}

/// A value on its way to a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub target: f32,
    /// Close enough to stop: within this of the target, and slower than ten
    /// times it a second.
    pub precision: f32,
}

impl Spring {
    /// At rest at `value`, for values about 0 to 1.
    pub fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.0,
            target: value,
            precision: 0.001,
        }
    }

    /// At rest at `value`, for pixels: stops within a twentieth of one.
    pub fn pixels(value: f32) -> Self {
        Self {
            precision: 0.05,
            ..Self::new(value)
        }
    }

    /// Aims at `target`, keeping its speed.
    pub fn to(&mut self, target: f32) {
        self.target = target;
    }

    /// There at once, at rest.
    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }

    pub fn is_moving(&self) -> bool {
        self.value != self.target || self.velocity != 0.0
    }

    /// Moves it `dt` seconds on. True while it still moves.
    pub fn step(&mut self, dt: f32, feel: &Feel) -> bool {
        if !self.is_moving() {
            return false;
        }
        if feel.mode == Mode::Off {
            self.snap(self.target);
            return false;
        }
        let (x, v) = solve(self.value - self.target, self.velocity, dt * feel.speed, feel.params);
        self.value = self.target + x;
        self.velocity = v;
        if x.abs() < self.precision && v.abs() < self.precision * 10.0 {
            self.snap(self.target);
        }
        self.is_moving()
    }
}

/// Where a damped spring is `t` seconds on, with `x` from its rest and `v`
/// its speed now: the exact solution, for each kind of damping.
fn solve(x: f32, v: f32, t: f32, params: SpringParams) -> (f32, f32) {
    let (x, v, t) = (x as f64, v as f64, t as f64);
    let w0 = (params.stiffness.max(1.0) as f64).sqrt();
    let z = params.damping.max(0.0) as f64;
    let (x, v) = if (z - 1.0).abs() < 1e-6 {
        let b = v + w0 * x;
        let e = (-w0 * t).exp();
        ((x + b * t) * e, (b - w0 * (x + b * t)) * e)
    } else if z < 1.0 {
        let wd = w0 * (1.0 - z * z).sqrt();
        let b = (v + z * w0 * x) / wd;
        let e = (-z * w0 * t).exp();
        let (s, c) = (wd * t).sin_cos();
        let position = e * (x * c + b * s);
        let speed = e * ((b * wd - z * w0 * x) * c - (x * wd + z * w0 * b) * s);
        (position, speed)
    } else {
        let root = (z * z - 1.0).sqrt();
        let (r1, r2) = (-w0 * (z - root), -w0 * (z + root));
        let c1 = (v - r2 * x) / (r1 - r2);
        let c2 = x - c1;
        let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
        (c1 * e1 + c2 * e2, c1 * r1 * e1 + c2 * r2 * e2)
    };
    (x as f32, v as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feel(preset: Preset) -> Feel {
        Feel {
            params: preset.params(),
            speed: 1.0,
            mode: Mode::Normal,
        }
    }

    /// Runs `spring` at `hz` until it rests; its peak and the time it took.
    fn run(spring: &mut Spring, feel: &Feel, hz: f32) -> (f32, f32) {
        let mut peak = spring.value;
        let mut time = 0.0;
        while spring.step(1.0 / hz, feel) {
            peak = peak.max(spring.value);
            time += 1.0 / hz;
            assert!(time < 10.0, "never rests");
        }
        (peak, time)
    }

    #[test]
    fn expressive_bounces_and_calm_does_not() {
        let mut spring = Spring::new(0.0);
        spring.to(1.0);
        let (peak, _) = run(&mut spring, &feel(Preset::Expressive), 240.0);
        assert!(peak > 1.02, "{peak}");
        let mut spring = Spring::new(0.0);
        spring.to(1.0);
        let (peak, _) = run(&mut spring, &feel(Preset::Calm), 240.0);
        assert!(peak <= 1.0 + 1e-4, "{peak}");
        assert_eq!(spring.value, 1.0);
    }

    #[test]
    fn the_same_motion_at_any_frame_rate() {
        for preset in Preset::ALL {
            let at = |hz: f32| {
                let mut spring = Spring::new(0.0);
                spring.to(1.0);
                let steps = (hz * 0.15) as usize;
                for _ in 0..steps {
                    spring.step(1.0 / hz, &feel(preset));
                }
                spring.value
            };
            assert!((at(60.0) - at(240.0)).abs() < 1e-3, "{preset:?}");
        }
    }

    #[test]
    fn a_new_target_keeps_the_speed() {
        let mut spring = Spring::new(0.0);
        spring.to(1.0);
        for _ in 0..10 {
            spring.step(1.0 / 240.0, &feel(Preset::Expressive));
        }
        let before = spring.velocity;
        spring.to(0.0);
        assert_eq!(spring.velocity, before);
        assert!(spring.velocity > 0.0);
    }

    #[test]
    fn off_is_instant_and_reduced_never_bounces() {
        let mut motion = Motion {
            mode: Mode::Off,
            ..Motion::default()
        };
        let mut spring = Spring::new(0.0);
        spring.to(1.0);
        assert!(!spring.step(1.0 / 60.0, &motion.feel(Part::Panels)));
        assert_eq!(spring.value, 1.0);
        motion.mode = Mode::Reduced;
        let mut spring = Spring::new(0.0);
        spring.to(1.0);
        let (peak, _) = run(&mut spring, &motion.feel(Part::Panels), 240.0);
        assert!(peak <= 1.0 + 1e-4, "{peak}");
    }

    #[test]
    fn parts_take_their_own_spring_or_the_global_one() {
        let motion = Motion::default();
        assert_eq!(motion.feel(Part::Panels).params, Preset::Expressive.params());
        assert_eq!(motion.feel(Part::Lists).params, Preset::Snappy.params());
        assert_eq!(motion.feel(Part::Panels).speed, 1.15);
    }

    #[test]
    fn the_solution_matches_small_steps() {
        for preset in Preset::ALL {
            let params = preset.params();
            let (exact, _) = solve(1.0, 0.0, 0.1, params);
            // Many tiny Euler steps come to the same place.
            let (mut x, mut v) = (1.0f64, 0.0f64);
            let c = 2.0 * params.damping as f64 * (params.stiffness as f64).sqrt();
            for _ in 0..100_000 {
                let a = -params.stiffness as f64 * x - c * v;
                v += a * 1e-6;
                x += v * 1e-6;
            }
            assert!((exact as f64 - x).abs() < 1e-3, "{preset:?}: {exact} vs {x}");
        }
    }
}
