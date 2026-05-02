use super::filters::FIRFilter;

// ── RDS standard constants ─────────────────────────────────────────────────────

const RDS_BIT_RATE: f32 = 1187.5; // bits per second

/// Lower 10 bits of generator polynomial G(x) = x^10 + x^8 + x^7 + x^5 + x^4 + x^3 + 1
const POLY_LOWER: u32 = 0x1B9;

/// Block offset words — syndrome of a valid received block equals its offset word
const OFFSET_A: u32 = 0x0FC;
const OFFSET_B: u32 = 0x198;
const OFFSET_C: u32 = 0x168;
const OFFSET_CP: u32 = 0x354; // C' used in version B groups
const OFFSET_D: u32 = 0x1B4;

// ── RDS decoder ────────────────────────────────────────────────────────────────

pub struct RdsDecoder {
    /// Low-pass filter for baseband RDS data (~2.4 kHz cutoff)
    bb_filter: FIRFilter,

    // Bit clock
    samples_per_bit: f32,
    // Initialized to T/4 so first fire lands at 3T/4 — optimal for biphase mark.
    // Biphase mark: "0"=one transition/bit (at boundary), "1"=two (boundary+mid).
    // At 3T/4 we are past any mid-bit transition and far from the next boundary.
    bit_phase: f32,

    // Previous filtered sample used for zero-crossing timing recovery
    prev_filtered: f32,

    // Differential biphase mark decoding
    prev_bit: u8,

    // 64-bit shift register — last 26 bits are checked for block sync
    shift_reg: u64,

    // Block accumulation state
    bits_since_sync: usize,
    block_idx: usize,
    group_blocks: [u16; 4],
    block_ok: [bool; 4],
    synced: bool,
    // Consecutive syndrome failures before dropping sync (allows 1 soft error)
    sync_errors: u8,

    // Parsed RDS data
    pub pi: u16,
    pub ps: [u8; 8],    // Programme Service name (8 chars)
    pub rt: [u8; 64],   // RadioText (up to 64 chars)
    pub rt_ab: bool,    // A/B flag — flips when RT changes
    pub pty: u8,        // Programme Type code (0-31)
    pub tp: bool,       // Traffic Programme
    pub ta: bool,       // Traffic Announcement
    pub ms: bool,       // Music/Speech: true = music
    pub ps_ready: bool, // at least one PS segment received
    pub rt_ready: bool, // CR (0x0D) end-of-text received
}

impl RdsDecoder {
    pub fn new(intermediate_rate: u32) -> Self {
        let samples_per_bit = intermediate_rate as f32 / RDS_BIT_RATE;
        let bb_cutoff = 2_400.0 / intermediate_rate as f32;
        Self {
            bb_filter: FIRFilter::new(design_low_pass(bb_cutoff, 128)),
            samples_per_bit,
            bit_phase: samples_per_bit * 0.25,
            prev_filtered: 0.0,
            prev_bit: 0,
            shift_reg: 0,
            bits_since_sync: 0,
            block_idx: 0,
            group_blocks: [0u16; 4],
            block_ok: [false; 4],
            synced: false,
            sync_errors: 0,
            pi: 0,
            ps: [0x20u8; 8],
            rt: [0x20u8; 64],
            rt_ab: false,
            pty: 0,
            tp: false,
            ta: false,
            ms: false,
            ps_ready: false,
            rt_ready: false,
        }
    }

    /// Feed one demodulated FM sample at intermediate_rate.
    /// `pilot_phase` is the 19 kHz pilot's current phase accumulator (radians).
    pub fn process(&mut self, fm_sample: f32, pilot_phase: f32) {
        // Coherent downmix: mix FM multiplex with 57 kHz reference (3 × pilot phase)
        let ref_57 = (pilot_phase * 3.0).cos();
        let mixed = fm_sample * ref_57;
        let baseband = self.bb_filter.process(mixed);

        // Timing recovery: bit boundaries produce zero crossings at bit_phase ≈ T/4
        // (because we sample at 3T/4; the boundary is T/4 after each sample).
        // Mid-bit crossings (for "1" bits) appear near bit_phase ≈ 3T/4 — ignore those.
        // Apply a 5% soft correction per detected boundary crossing to track drift.
        if self.prev_filtered * baseband < 0.0 {
            let norm = self.bit_phase / self.samples_per_bit;
            if norm > 0.05 && norm < 0.50 {
                let err = norm - 0.25;
                self.bit_phase -= err * self.samples_per_bit * 0.05;
                self.bit_phase = self.bit_phase.max(0.0);
            }
        }
        self.prev_filtered = baseband;

        self.bit_phase += 1.0;
        if self.bit_phase >= self.samples_per_bit {
            self.bit_phase -= self.samples_per_bit;
            self.clock_bit(baseband);
        }
    }

    fn clock_bit(&mut self, sample: f32) {
        // Hard decision on baseband level
        let bit: u8 = if sample >= 0.0 { 1 } else { 0 };
        // Differential biphase mark decode: 1 = level change, 0 = same level
        let decoded = (bit ^ self.prev_bit) ^ 1;
        self.prev_bit = bit;
        self.push_bit(decoded);
    }

    fn push_bit(&mut self, bit: u8) {
        self.shift_reg = (self.shift_reg << 1) | (bit as u64 & 1);

        if !self.synced {
            // Check the last 26 bits against all valid syndromes
            let word = (self.shift_reg & 0x3FF_FFFF) as u32;
            let syn = syndrome(word);
            if matches!(syn, OFFSET_A | OFFSET_B | OFFSET_C | OFFSET_CP | OFFSET_D) {
                self.synced = true;
                self.bits_since_sync = 0;
                // Infer which block we just completed
                self.block_idx = match syn {
                    OFFSET_A => 0,
                    OFFSET_B => 1,
                    OFFSET_C | OFFSET_CP => 2,
                    _ => 3,
                };
                self.group_blocks[self.block_idx] = ((word >> 10) & 0xFFFF) as u16;
                self.block_ok[self.block_idx] = true;
                self.block_idx = (self.block_idx + 1) % 4;
            }
            return;
        }

        self.bits_since_sync += 1;
        if self.bits_since_sync < 26 {
            return;
        }
        self.bits_since_sync = 0;

        let word = (self.shift_reg & 0x3FF_FFFF) as u32;
        let syn = syndrome(word);
        let data = ((word >> 10) & 0xFFFF) as u16;

        let expected = match self.block_idx {
            0 => OFFSET_A,
            1 => OFFSET_B,
            2 => OFFSET_C,
            _ => OFFSET_D,
        };
        let also_c_prime = self.block_idx == 2 && syn == OFFSET_CP;
        let ok = syn == expected || also_c_prime;

        if !ok {
            self.sync_errors += 1;
            if self.sync_errors >= 2 {
                // Two consecutive bad blocks — truly lost framing
                self.synced = false;
                self.block_idx = 0;
                self.block_ok = [false; 4];
                self.sync_errors = 0;
            } else {
                // One bad block — mark it invalid, advance framing, keep going
                self.block_ok[self.block_idx] = false;
                self.block_idx = (self.block_idx + 1) % 4;
                self.bits_since_sync = 0;
            }
            return;
        }
        self.sync_errors = 0;

        self.group_blocks[self.block_idx] = data;
        self.block_ok[self.block_idx] = true;
        self.block_idx += 1;

        if self.block_idx == 4 {
            self.block_idx = 0;
            self.parse_group();
            self.block_ok = [false; 4];
        }
    }

    fn parse_group(&mut self) {
        // Blocks 0 and 1 must be error-free to determine group type
        if !self.block_ok[0] || !self.block_ok[1] {
            return;
        }

        self.pi = self.group_blocks[0];
        let b1 = self.group_blocks[1];

        let group_type = (b1 >> 12) & 0xF;
        let version = (b1 >> 11) & 1; // 0 = version A, 1 = version B
        self.tp = (b1 >> 10) & 1 != 0;
        self.pty = ((b1 >> 5) & 0x1F) as u8;

        match (group_type, version) {
            (0, _) => self.parse_group_0a(b1),
            (2, 0) => self.parse_group_2a(b1),
            (2, 1) => self.parse_group_2b(b1),
            _ => {} // 1A (AF list), 4A (clock time), etc. — ignored for now
        }
    }

    /// Group 0A/0B — Programme Service name (2 chars per group, 4 groups = 8 chars)
    fn parse_group_0a(&mut self, b1: u16) {
        if !self.block_ok[3] {
            return;
        }
        self.ta = (b1 >> 4) & 1 != 0;
        self.ms = (b1 >> 3) & 1 != 0;
        let seg = (b1 & 0x3) as usize;
        let d = self.group_blocks[3];
        let c0 = (d >> 8) as u8;
        let c1 = (d & 0xFF) as u8;
        if c0 >= 0x20 && c0 < 0x7F {
            self.ps[seg * 2] = c0;
        }
        if c1 >= 0x20 && c1 < 0x7F {
            self.ps[seg * 2 + 1] = c1;
        }
        self.ps_ready = self.ps.iter().any(|&b| b > 0x20);
    }

    /// Group 2A — RadioText (4 chars per group, up to 16 groups = 64 chars)
    fn parse_group_2a(&mut self, b1: u16) {
        if !self.block_ok[2] || !self.block_ok[3] {
            return;
        }
        let ab = (b1 >> 4) & 1 != 0;
        if ab != self.rt_ab {
            self.rt = [0x20u8; 64];
            self.rt_ab = ab;
            self.rt_ready = false;
        }
        let seg = (b1 & 0xF) as usize;
        let base = seg * 4;
        let words = [self.group_blocks[2], self.group_blocks[3]];
        for (wi, &w) in words.iter().enumerate() {
            let c0 = (w >> 8) as u8;
            let c1 = (w & 0xFF) as u8;
            let off = base + wi * 2;
            if off + 1 < 64 {
                if c0 == 0x0D {
                    self.rt_ready = true;
                    return;
                }
                self.rt[off] = if c0 >= 0x20 && c0 < 0x7F { c0 } else { 0x20 };
                if c1 == 0x0D {
                    self.rt_ready = true;
                    return;
                }
                self.rt[off + 1] = if c1 >= 0x20 && c1 < 0x7F { c1 } else { 0x20 };
            }
        }
    }

    /// Group 2B — RadioText (2 chars per group, up to 16 groups = 32 chars)
    fn parse_group_2b(&mut self, b1: u16) {
        if !self.block_ok[3] {
            return;
        }
        let ab = (b1 >> 4) & 1 != 0;
        if ab != self.rt_ab {
            self.rt = [0x20u8; 64];
            self.rt_ab = ab;
            self.rt_ready = false;
        }
        let seg = (b1 & 0xF) as usize;
        let d = self.group_blocks[3];
        let c0 = (d >> 8) as u8;
        let c1 = (d & 0xFF) as u8;
        let off = seg * 2;
        if off + 1 < 32 {
            self.rt[off] = if c0 >= 0x20 && c0 < 0x7F { c0 } else { 0x20 };
            self.rt[off + 1] = if c1 >= 0x20 && c1 < 0x7F { c1 } else { 0x20 };
        }
    }

    pub fn ps_string(&self) -> String {
        String::from_utf8_lossy(&self.ps).trim_end().to_string()
    }

    pub fn rt_string(&self) -> String {
        let raw = String::from_utf8_lossy(&self.rt);
        // Trim trailing spaces and control characters
        raw.trim_end_matches(|c: char| c == ' ' || c < ' ')
            .to_string()
    }
}

// ── CRC-10 syndrome ────────────────────────────────────────────────────────────

/// Compute the CRC-10 syndrome of a 26-bit RDS codeword [data(16) | checkword(10)].
/// A valid block's syndrome equals its offset word.
fn syndrome(word: u32) -> u32 {
    let mut reg: u32 = 0;
    for i in (0..26).rev() {
        let bit = (word >> i) & 1;
        let feedback = bit ^ ((reg >> 9) & 1);
        reg = ((reg << 1) & 0x3FF) ^ (if feedback != 0 { POLY_LOWER } else { 0 });
    }
    reg
}
