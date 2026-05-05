use futuredsp::Filter;
use num_complex::Complex;

type Cf32 = Complex<f32>;

// ── Design ────────────────────────────────────────────────────────────────────

pub mod firdes {
    pub struct Kaiser(f32);

    impl Kaiser {
        pub fn new(atten_db: f32) -> Self {
            Self(atten_db)
        }
    }

    /// Kaiser-windowed lowpass FIR coefficients.
    /// `cutoff_norm` — normalised cutoff (0 = DC, 0.5 = Nyquist).
    pub fn lowpass(cutoff_norm: f32, window: Kaiser) -> Vec<f32> {
        let max_ripple = 10_f64.powf(-(window.0 as f64) / 20.0);
        let trans_bw = (cutoff_norm as f64 * 0.5).clamp(0.01, 0.1);
        futuredsp::firdes::kaiser::lowpass::<f32>(cutoff_norm as f64, trans_bw, max_ripple)
    }
}

// ── Block-stage FIR (f32 → f32) ──────────────────────────────────────────────
//
// Overlap-save: prepend N-1 history samples before each block so the stateless
// futuredsp kernel produces exactly `input.len()` outputs per call.

pub struct FirFilter {
    inner: futuredsp::FirFilter<f32, f32, Vec<f32>>,
    history: Vec<f32>,
    ext_buf: Vec<f32>,
}

impl FirFilter {
    pub fn new(taps: Vec<f32>) -> Self {
        let overlap = taps.len().saturating_sub(1);
        Self {
            inner: futuredsp::FirFilter::new(taps),
            history: vec![0.0; overlap],
            ext_buf: Vec::new(),
        }
    }

    /// Replace taps in-place (e.g. bandwidth switch). History is preserved for
    /// continuity; length is padded or truncated to match the new tap count.
    pub fn set_taps(&mut self, taps: Vec<f32>) {
        let new_overlap = taps.len().saturating_sub(1);
        self.inner = futuredsp::FirFilter::new(taps);
        self.history.resize(new_overlap, 0.0);
    }

    pub fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let h = self.history.len();
        self.ext_buf.clear();
        self.ext_buf.extend_from_slice(&self.history);
        self.ext_buf.extend_from_slice(input);

        out.resize(input.len(), 0.0);
        self.inner.filter(&self.ext_buf, out);

        let start = self.ext_buf.len() - h;
        self.history.copy_from_slice(&self.ext_buf[start..]);
    }
}

// ── Block-stage decimating FIR (f32 → f32) ───────────────────────────────────

pub struct DecimatingFirFilter {
    inner: futuredsp::DecimatingFirFilter<f32, f32, Vec<f32>>,
    history: Vec<f32>,
    decimation: usize,
    ext_buf: Vec<f32>,
}

impl DecimatingFirFilter {
    pub fn new(decimation: usize, taps: Vec<f32>) -> Self {
        let overlap = taps.len().saturating_sub(1);
        Self {
            inner: futuredsp::DecimatingFirFilter::new(decimation, taps),
            history: vec![0.0; overlap],
            decimation,
            ext_buf: Vec::new(),
        }
    }

    /// Produces `floor(input.len() / decimation)` samples.
    /// Leftover input is captured in history for the next call.
    pub fn process_into(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let h = self.history.len();
        self.ext_buf.clear();
        self.ext_buf.extend_from_slice(&self.history);
        self.ext_buf.extend_from_slice(input);

        let out_len = input.len() / self.decimation;
        out.resize(out_len, 0.0);
        self.inner.filter(&self.ext_buf, out);

        let start = self.ext_buf.len() - h;
        self.history.copy_from_slice(&self.ext_buf[start..]);
    }
}

// ── Block-stage decimating FIR (Cf32 → Cf32, real f32 taps) ─────────────────
//
// Real taps applied symmetrically to I and Q — used for IQ pre-filtering in
// the AM pipeline (replaces two separate f32 filters).

pub struct ComplexDecimatingFirFilter {
    inner: futuredsp::DecimatingFirFilter<Cf32, Cf32, Vec<f32>>,
    history: Vec<Cf32>,
    decimation: usize,
    ext_buf: Vec<Cf32>,
}

impl ComplexDecimatingFirFilter {
    pub fn new(decimation: usize, taps: Vec<f32>) -> Self {
        let overlap = taps.len().saturating_sub(1);
        Self {
            inner: futuredsp::DecimatingFirFilter::new(decimation, taps),
            history: vec![Cf32::new(0.0, 0.0); overlap],
            decimation,
            ext_buf: Vec::new(),
        }
    }

    pub fn process_into(&mut self, input: &[Cf32], out: &mut Vec<Cf32>) {
        let h = self.history.len();
        self.ext_buf.clear();
        self.ext_buf.extend_from_slice(&self.history);
        self.ext_buf.extend_from_slice(input);

        let out_len = input.len() / self.decimation;
        out.resize(out_len, Cf32::new(0.0, 0.0));
        self.inner.filter(&self.ext_buf, out);

        let start = self.ext_buf.len() - h;
        self.history.copy_from_slice(&self.ext_buf[start..]);
    }
}
