#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowpass_passband_unity() {
        let spec = FilterSpec::new(127, Window::Hamming);
        let coeffs = design_lowpass(0.1, &spec);
        let gain_db = frequency_response_db(&coeffs, 0.05); // well inside passband
        assert!(
            (gain_db - 0.0).abs() < 1.0,
            "passband gain {gain_db:.1}dB, expected ~0dB"
        );
    }

    #[test]
    fn lowpass_stopband_attenuated() {
        let spec = FilterSpec::new(127, Window::Hamming);
        let coeffs = design_lowpass(0.1, &spec);
        let gain_db = frequency_response_db(&coeffs, 0.25); // well into stopband
        assert!(
            gain_db < -40.0,
            "stopband gain {gain_db:.1}dB, expected < -40dB"
        );
    }

    #[test]
    fn decimating_filter_no_alias() {
        // 2.4MSPS → 240kHz, verify signal at 130kHz (above output Nyquist) is killed
        let mut d = DecimatingFilter::new(100_000.0, 2_400_000, 240_000, Window::Kaiser(6.0));
        let gain = frequency_response(
            &d.filter.coefficients,
            normalize_freq(130_000.0, 2_400_000.0),
        );
        assert!(gain < 0.01, "alias frequency not attenuated: {gain:.4}");
    }
}
