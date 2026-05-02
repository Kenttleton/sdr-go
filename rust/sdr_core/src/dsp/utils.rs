pub fn normalize_freq(hz: f32, sample_rate: f32) -> f32 {
    hz / sample_rate
}

pub fn estimate_taps(transition_hz: f32, sample_rate: f32, attenuation_db: f32) -> usize {
    let transition_normalized = transition_hz / sample_rate;
    // Harris approximation
    let taps = (attenuation_db / (22.0 * transition_normalized)).ceil() as usize;
    if taps % 2 == 0 {
        taps + 1
    } else {
        taps
    }
}

pub fn frequency_response(coeffs: &[f32], freq_normalized: f32) -> f32 {
    let n = coeffs.len();
    let mut re = 0.0f32;
    let mut im = 0.0f32;
    for (k, &c) in coeffs.iter().enumerate() {
        let angle = -2.0 * PI * freq_normalized * k as f32;
        re += c * angle.cos();
        im += c * angle.sin();
    }
    (re * re + im * im).sqrt()
}

pub fn frequency_response_db(coeffs: &[f32], freq_normalized: f32) -> f32 {
    20.0 * frequency_response(coeffs, freq_normalized)
        .max(1e-10)
        .log10()
}
