// WFM demod: 200kHz channel, 2.4MSPS → transition ~100kHz → ~12 taps (very cheap)
// NFM: 12.5kHz channel from 240kHz IQ → transition ~5kHz → ~50-80 taps
// AM audio LPF at 5kHz from 48kHz audio → transition ~2kHz → ~120 taps

pub enum DemodulationMode {
    WFM,
    NFM,
    AM,
    LSB,
    USB,
    DSB,
    RAW,
}

pub enum Deemphesis {
    Stereo,
    Mono,
}
