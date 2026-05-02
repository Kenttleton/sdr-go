pub struct Agc {
    pub gain: f32,
    target: f32,
    attack: f32,
    decay: f32,
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
        // attack: ~48ms to -6dB — slow enough not to crush transients
        // decay: ~500ms to +6dB — prevents over-gain on quiet passages before a loud section
        // target: 0.3 leaves headroom; audio clips at 1.0 and RTL-SDR auto-gain is unpredictable
        Self::new(0.3, 0.0002, 0.00002)
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

        // Clamp gain — upper limit prevents over-gain on quiet passages from
        // causing clipping when a loud section arrives before AGC responds.
        self.gain = self.gain.clamp(0.001, 10.0);
        output
    }

    pub fn process_block(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process(*s);
        }
    }
}