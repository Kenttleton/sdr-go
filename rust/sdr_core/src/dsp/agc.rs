/// Automatic Gain Control
/// Normalizes signal amplitude so downstream demodulators
/// don't need to worry about signal strength variation
pub struct Agc {
    gain: f32,
    target: f32,
    attack: f32,   // how fast gain decreases on loud signals
    decay: f32,    // how fast gain increases on quiet signals
}

impl Agc {
    pub fn new(target: f32, attack: f32, decay: f32) -> Self {
        Self {
            gain: 1.0,
            target,
            attack,
            decay,
        }
    }

    /// Sensible defaults for FM broadcast
    pub fn default_fm() -> Self {
        Self::new(0.5, 0.001, 0.0001)
    }

    /// Sensible defaults for air band AM
    pub fn default_am() -> Self {
        Self::new(0.5, 0.01, 0.001)
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let output = sample * self.gain;
        let amplitude = output.abs();

        // Adjust gain toward target amplitude
        if amplitude > self.target {
            self.gain *= 1.0 - self.attack;
        } else {
            self.gain *= 1.0 + self.decay;
        }

        // Clamp gain to sane range — prevents runaway on silence
        self.gain = self.gain.clamp(0.001, 1000.0);
        output
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process(*s);
        }
    }
}