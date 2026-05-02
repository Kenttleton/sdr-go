use std::f32::consts::PI;

pub enum Window {
    Rectangular,
    Hann,
    Hamming,
    Blackman,
    BlackmanHarris,
    Kaiser(f32),
}

impl Window {
    pub fn apply(&self, coeffs: &mut [f32]) {
        let n = coeffs.len();
        let m = (n - 1) as f32;

        for (i, c) in coeffs.iter_mut().enumerate() {
            let ratio = i as f32 / m;
            let w = match self {
                Window::Rectangular => 1.0,
                Window::Hann => 0.5 * (1.0 - (2.0 * PI * ratio).cos()),
                Window::Hamming => 0.54 - 0.46 * (2.0 * PI * ratio).cos(),
                Window::Blackman => {
                    0.42 - 0.5 * (2.0 * PI * ratio).cos() + 0.08 * (4.0 * PI * ratio).cos()
                }
                Window::BlackmanHarris => {
                    0.35875 - 0.48829 * (2.0 * PI * ratio).cos()
                        + 0.14128 * (4.0 * PI * ratio).cos()
                        - 0.01168 * (6.0 * PI * ratio).cos()
                }
                Window::Kaiser(beta) => {
                    bessel_i0(*beta * (1.0 - (2.0 * ratio - 1.0).powi(2)).sqrt()) / bessel_i0(*beta)
                }
            };
            *c *= w;
        }
    }
}

// Modified Bessel function I0 — needed for Kaiser window
fn bessel_i0(x: f32) -> f32 {
    let mut sum = 1.0f32;
    let mut term = 1.0f32;
    for k in 1..=20 {
        term *= (x / (2.0 * k as f32)).powi(2);
        sum += term;
        if term < 1e-12 {
            break;
        }
    }
    sum
}
