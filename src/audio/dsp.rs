use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use std::f32::consts::{PI, SQRT_2};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub const SAMPLE_RATE: u32 = 48_000;
const PITCH_FRAME: usize = 1024;
const PITCH_OSAMP: usize = 4;

#[derive(Default)]
pub struct Params {
    pub pitch_enabled: AtomicBool,
    pub pitch_semitones: AtomicU32,
    pub formant_shift: AtomicU32,
    pub eq_low_db: AtomicU32,
    pub eq_mid_freq: AtomicU32,
    pub eq_mid_gain_db: AtomicU32,
    pub eq_mid_q: AtomicU32,
    pub eq_high_db: AtomicU32,
    pub radio_enabled: AtomicBool,
    pub radio_bp_low_hz: AtomicU32,
    pub radio_bp_high_hz: AtomicU32,
    pub radio_drive: AtomicU32,
    pub radio_noise_db: AtomicU32,
    pub ducking_enabled: AtomicBool,
    pub duck_atten_db: AtomicU32,
    pub duck_release_ms: AtomicU32,
    pub monitor_volume: AtomicU32,
    pub monitor_enabled: AtomicBool,
    pub stream_volume: AtomicU32,
    pub noise_gate_enabled: AtomicBool,
    pub noise_gate_db: AtomicU32,
}

fn load(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

fn store(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

impl Params {
    pub fn with_snapshot(snap: &DspSnapshot) -> Arc<Self> {
        let p = Self::default();
        p.apply(snap);
        Arc::new(p)
    }

    pub fn apply(&self, s: &DspSnapshot) {
        self.pitch_enabled.store(s.pitch_enabled, Ordering::Relaxed);
        store(&self.pitch_semitones, s.pitch_semitones);
        store(&self.formant_shift, s.formant_shift);
        store(&self.eq_low_db, s.eq_low_db);
        store(&self.eq_mid_freq, s.eq_mid_freq);
        store(&self.eq_mid_gain_db, s.eq_mid_gain_db);
        store(&self.eq_mid_q, s.eq_mid_q);
        store(&self.eq_high_db, s.eq_high_db);
        self.radio_enabled.store(s.radio_enabled, Ordering::Relaxed);
        store(&self.radio_bp_low_hz, s.radio_bp_low_hz);
        store(&self.radio_bp_high_hz, s.radio_bp_high_hz);
        store(&self.radio_drive, s.radio_drive);
        store(&self.radio_noise_db, s.radio_noise_db);
        self.ducking_enabled.store(s.ducking_enabled, Ordering::Relaxed);
        store(&self.duck_atten_db, s.duck_atten_db);
        store(&self.duck_release_ms, s.duck_release_ms);
        store(&self.monitor_volume, s.monitor_volume);
        self.monitor_enabled.store(s.monitor_enabled, Ordering::Relaxed);
        store(&self.stream_volume, s.stream_volume);
        self.noise_gate_enabled.store(s.noise_gate_enabled, Ordering::Relaxed);
        store(&self.noise_gate_db, s.noise_gate_db);
    }

    pub fn snapshot(&self) -> DspSnapshot {
        DspSnapshot {
            pitch_enabled: self.pitch_enabled.load(Ordering::Relaxed),
            pitch_semitones: load(&self.pitch_semitones).clamp(-12.0, 12.0),
            formant_shift: load(&self.formant_shift).clamp(0.5, 2.0),
            eq_low_db: load(&self.eq_low_db).clamp(-24.0, 24.0),
            eq_mid_freq: load(&self.eq_mid_freq).clamp(60.0, 12_000.0),
            eq_mid_gain_db: load(&self.eq_mid_gain_db).clamp(-24.0, 24.0),
            eq_mid_q: load(&self.eq_mid_q).clamp(0.2, 8.0),
            eq_high_db: load(&self.eq_high_db).clamp(-24.0, 24.0),
            radio_enabled: self.radio_enabled.load(Ordering::Relaxed),
            radio_bp_low_hz: load(&self.radio_bp_low_hz).clamp(80.0, 2000.0),
            radio_bp_high_hz: load(&self.radio_bp_high_hz).clamp(1500.0, 12_000.0),
            radio_drive: load(&self.radio_drive).clamp(0.0, 1.0),
            radio_noise_db: load(&self.radio_noise_db).clamp(-96.0, -20.0),
            ducking_enabled: self.ducking_enabled.load(Ordering::Relaxed),
            duck_atten_db: load(&self.duck_atten_db).clamp(-60.0, 0.0),
            duck_release_ms: load(&self.duck_release_ms).clamp(20.0, 2000.0),
            monitor_volume: load(&self.monitor_volume).clamp(0.0, 1.0),
            monitor_enabled: self.monitor_enabled.load(Ordering::Relaxed),
            stream_volume: load(&self.stream_volume).clamp(0.0, 2.0),
            noise_gate_enabled: self.noise_gate_enabled.load(Ordering::Relaxed),
            noise_gate_db: load(&self.noise_gate_db).clamp(-90.0, -20.0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DspSnapshot {
    pub pitch_enabled: bool,
    pub pitch_semitones: f32,
    pub formant_shift: f32,
    pub eq_low_db: f32,
    pub eq_mid_freq: f32,
    pub eq_mid_gain_db: f32,
    pub eq_mid_q: f32,
    pub eq_high_db: f32,
    pub radio_enabled: bool,
    pub radio_bp_low_hz: f32,
    pub radio_bp_high_hz: f32,
    pub radio_drive: f32,
    pub radio_noise_db: f32,
    pub ducking_enabled: bool,
    pub duck_atten_db: f32,
    pub duck_release_ms: f32,
    pub monitor_volume: f32,
    pub monitor_enabled: bool,
    pub stream_volume: f32,
    pub noise_gate_enabled: bool,
    pub noise_gate_db: f32,
}

impl Default for DspSnapshot {
    fn default() -> Self {
        Self {
            pitch_enabled: false,
            pitch_semitones: 0.0,
            formant_shift: 1.0,
            eq_low_db: 0.0,
            eq_mid_freq: 1200.0,
            eq_mid_gain_db: 0.0,
            eq_mid_q: 1.2,
            eq_high_db: 0.0,
            radio_enabled: false,
            radio_bp_low_hz: 380.0,
            radio_bp_high_hz: 3600.0,
            radio_drive: 0.0,
            radio_noise_db: -96.0,
            ducking_enabled: true,
            duck_atten_db: -12.0,
            duck_release_ms: 120.0,
            monitor_volume: 1.0,
            monitor_enabled: false,
            stream_volume: 1.0,
            noise_gate_enabled: true,
            noise_gate_db: -52.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn from_coeffs(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            ..Self::identity()
        }
    }

    pub fn peaking(fs: f32, f0: f32, gain_db: f32, q: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / (2.0 * q);
        Self::from_coeffs(
            1.0 + alpha * a,
            -2.0 * w0.cos(),
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * w0.cos(),
            1.0 - alpha / a,
        )
    }

    pub fn low_shelf(fs: f32, f0: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / 2.0 * SQRT_2;
        let cos = w0.cos();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        Self::from_coeffs(
            a * ((a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha,
        )
    }

    pub fn high_shelf(fs: f32, f0: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / 2.0 * SQRT_2;
        let cos = w0.cos();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        Self::from_coeffs(
            a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha,
        )
    }

    pub fn lowpass(fs: f32, f0: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / (2.0 * q);
        Self::from_coeffs(
            (1.0 - w0.cos()) / 2.0,
            1.0 - w0.cos(),
            (1.0 - w0.cos()) / 2.0,
            1.0 + alpha,
            -2.0 * w0.cos(),
            1.0 - alpha,
        )
    }

    pub fn highpass(fs: f32, f0: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / (2.0 * q);
        Self::from_coeffs(
            (1.0 + w0.cos()) / 2.0,
            -(1.0 + w0.cos()),
            (1.0 + w0.cos()) / 2.0,
            1.0 + alpha,
            -2.0 * w0.cos(),
            1.0 - alpha,
        )
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct ParametricEq {
    low: Biquad,
    mid: Biquad,
    presence: Biquad,
    high: Biquad,
    key: [u32; 5],
}

impl ParametricEq {
    pub fn new() -> Self {
        Self {
            low: Biquad::identity(),
            mid: Biquad::identity(),
            presence: Biquad::identity(),
            high: Biquad::identity(),
            key: [0; 5],
        }
    }

    pub fn update(&mut self, s: &DspSnapshot) {
        let fs = SAMPLE_RATE as f32;
        let key = [
            s.eq_low_db.to_bits(),
            s.eq_mid_freq.to_bits(),
            s.eq_mid_gain_db.to_bits(),
            s.eq_mid_q.to_bits(),
            s.eq_high_db.to_bits(),
        ];
        if key == self.key {
            return;
        }
        self.key = key;
        let presence_freq = (s.eq_mid_freq * 2.0).min(14_000.0);
        self.low = Biquad::low_shelf(fs, 200.0, s.eq_low_db);
        self.mid = Biquad::peaking(fs, s.eq_mid_freq, s.eq_mid_gain_db, s.eq_mid_q);
        self.presence = Biquad::peaking(fs, presence_freq, s.eq_mid_gain_db * 0.5, s.eq_mid_q * 0.8);
        self.high = Biquad::high_shelf(fs, 6000.0, s.eq_high_db);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.low.process(x);
        let y = self.mid.process(y);
        let y = self.presence.process(y);
        self.high.process(y)
    }
}

pub struct PhaseVocoder {
    frame: usize,
    half: usize,
    step: usize,
    latency: usize,
    freq_per_bin: f32,
    expct: f32,
    in_fifo: Vec<f32>,
    out_fifo: Vec<f32>,
    window: Vec<f32>,
    last_phase: Vec<f32>,
    sum_phase: Vec<f32>,
    accum: Vec<f32>,
    ana_magn: Vec<f32>,
    ana_freq: Vec<f32>,
    syn_magn: Vec<f32>,
    syn_freq: Vec<f32>,
    fft_buf: Vec<Complex32>,
    scratch: Vec<Complex32>,
    fft_fwd: Arc<dyn Fft<f32>>,
    fft_inv: Arc<dyn Fft<f32>>,
    rover: usize,
    ratio: f32,
}

impl PhaseVocoder {
    pub fn new(sample_rate: u32) -> Self {
        let frame = PITCH_FRAME;
        let half = frame / 2;
        let step = frame / PITCH_OSAMP;
        let latency = frame - step;
        let mut planner = FftPlanner::<f32>::new();
        let fft_fwd = planner.plan_fft_forward(frame);
        let fft_inv = planner.plan_fft_inverse(frame);
        let scratch_len = fft_fwd
            .get_inplace_scratch_len()
            .max(fft_inv.get_inplace_scratch_len());
        Self {
            frame,
            half,
            step,
            latency,
            freq_per_bin: sample_rate as f32 / frame as f32,
            expct: 2.0 * PI * step as f32 / frame as f32,
            in_fifo: vec![0.0; frame],
            out_fifo: vec![0.0; frame],
            window: (0..frame)
                .map(|k| -0.5 * (2.0 * PI * k as f32 / frame as f32).cos() + 0.5)
                .collect(),
            last_phase: vec![0.0; half + 1],
            sum_phase: vec![0.0; half + 1],
            accum: vec![0.0; 2 * frame],
            ana_magn: vec![0.0; half + 1],
            ana_freq: vec![0.0; half + 1],
            syn_magn: vec![0.0; half + 1],
            syn_freq: vec![0.0; half + 1],
            fft_buf: vec![Complex32::ZERO; frame],
            scratch: vec![Complex32::ZERO; scratch_len],
            fft_fwd,
            fft_inv,
            rover: latency,
            ratio: 1.0,
        }
    }

    pub fn set_semitones(&mut self, semitones: f32) {
        self.ratio = 2f32.powf(semitones.clamp(-12.0, 12.0) / 12.0);
    }

    pub fn reset(&mut self) {
        self.in_fifo.fill(0.0);
        self.out_fifo.fill(0.0);
        self.accum.fill(0.0);
        self.last_phase.fill(0.0);
        self.sum_phase.fill(0.0);
        self.rover = self.latency;
    }

    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.in_fifo[self.rover] = x;
        let y = self.out_fifo[self.rover - self.latency];
        self.rover += 1;
        if self.rover >= self.frame {
            self.rover = self.latency;
            self.shift_block();
        }
        y
    }

    fn shift_block(&mut self) {
        let frame = self.frame;
        let half = self.half;
        let step = self.step;
        let osamp = PITCH_OSAMP as f32;
        let ratio = self.ratio;

        for k in 0..frame {
            self.fft_buf[k] = Complex32::new(self.window[k] * self.in_fifo[k], 0.0);
        }
        self.fft_fwd
            .process_with_scratch(&mut self.fft_buf, &mut self.scratch);

        for k in 0..=half {
            let re = self.fft_buf[k].re;
            let im = self.fft_buf[k].im;
            let magn = 2.0 * (re * re + im * im).sqrt();
            let phase = im.atan2(re);

            let mut tmp = phase - self.last_phase[k];
            self.last_phase[k] = phase;
            tmp -= k as f32 * self.expct;

            let qpd = (tmp / PI) as i32;
            let qpd = if qpd >= 0 { qpd + (qpd & 1) } else { qpd - (qpd & 1) };
            tmp -= PI * qpd as f32;

            tmp = tmp * osamp / (2.0 * PI);
            self.ana_magn[k] = magn;
            self.ana_freq[k] = (k as f32 + tmp) * self.freq_per_bin;
        }

        self.syn_magn.fill(0.0);
        self.syn_freq.fill(0.0);
        for k in 0..=half {
            let index = (k as f32 * ratio) as usize;
            if index <= half {
                self.syn_magn[index] += self.ana_magn[k];
                self.syn_freq[index] = self.ana_freq[k] * ratio;
            }
        }
        for k in 0..=half {
            let magn = self.syn_magn[k];
            let mut tmp = self.syn_freq[k];
            tmp -= k as f32 * self.freq_per_bin;
            tmp /= self.freq_per_bin;
            tmp = 2.0 * PI * tmp / osamp;
            tmp += k as f32 * self.expct;
            self.sum_phase[k] += tmp;
            let phase = self.sum_phase[k];
            self.fft_buf[k] = Complex32::new(magn * phase.cos(), magn * phase.sin());
        }
        for k in (half + 1)..frame {
            self.fft_buf[k] = Complex32::ZERO;
        }
        self.fft_inv
            .process_with_scratch(&mut self.fft_buf, &mut self.scratch);

        let scale = 2.0 / (half as f32 * osamp);
        for k in 0..frame {
            self.accum[k] += self.window[k] * self.fft_buf[k].re * scale;
        }

        self.out_fifo[..step].copy_from_slice(&self.accum[..step]);
        self.accum.copy_within(step..2 * frame, 0);
        for k in (2 * frame - step)..2 * frame {
            self.accum[k] = 0.0;
        }
        self.in_fifo.copy_within(step..frame, 0);
    }
}

pub struct FormantShifter {
    f1: Biquad,
    f2: Biquad,
    current: f32,
}

impl FormantShifter {
    pub fn new() -> Self {
        Self {
            f1: Biquad::identity(),
            f2: Biquad::identity(),
            current: 1.0,
        }
    }

    pub fn update(&mut self, shift: f32) {
        if (shift - self.current).abs() < 0.001 {
            return;
        }
        self.current = shift;
        let fs = SAMPLE_RATE as f32;
        let depth = (1.0 - shift).clamp(-1.0, 1.0) * 6.0;
        self.f1 = Biquad::peaking(fs, 850.0, depth, 1.1);
        self.f2 = Biquad::peaking(fs, 2500.0, -depth, 1.3);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.f2.process(self.f1.process(x))
    }
}

pub struct RadioFx {
    hp: Biquad,
    lp: Biquad,
    key: [u32; 2],
    drive: f32,
    noise_amp: f32,
    noise_state: u32,
}

impl RadioFx {
    pub fn new() -> Self {
        Self {
            hp: Biquad::identity(),
            lp: Biquad::identity(),
            key: [0; 2],
            drive: 1.0,
            noise_amp: 0.0,
            noise_state: 0x2545_F491,
        }
    }

    pub fn update(&mut self, s: &DspSnapshot) {
        let key = [s.radio_bp_low_hz.to_bits(), s.radio_bp_high_hz.to_bits()];
        if key != self.key {
            self.key = key;
            let fs = SAMPLE_RATE as f32;
            self.hp = Biquad::highpass(fs, s.radio_bp_low_hz, 0.707);
            self.lp = Biquad::lowpass(fs, s.radio_bp_high_hz, 0.707);
        }
        self.drive = 1.0 + s.radio_drive * 9.0;
        self.noise_amp = if s.radio_noise_db > -60.0 {
            10f32.powf(s.radio_noise_db / 20.0)
        } else {
            0.0
        };
    }

    #[inline]
    fn noise(&mut self) -> f32 {
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let band = self.lp.process(self.hp.process(x));
        let driven = (band * self.drive).tanh() / self.drive.tanh();
        if self.noise_amp > 0.0 && band.abs() > 0.002 {
            driven + self.noise() * self.noise_amp
        } else {
            driven
        }
    }
}

pub struct DuckEnvelope {
    gain: f32,
}

impl DuckEnvelope {
    pub fn new() -> Self {
        Self { gain: 1.0 }
    }

    pub fn next_gain(&mut self, pad_active: bool, s: &DspSnapshot, block_len: usize) -> f32 {
        let target = if pad_active && s.ducking_enabled {
            10f32.powf(s.duck_atten_db / 20.0)
        } else {
            1.0
        };
        let tau_ms = if target < self.gain { 50.0 } else { s.duck_release_ms };
        let coef = (-(block_len as f32) / (tau_ms * 0.001 * SAMPLE_RATE as f32)).exp();
        self.gain = target + (self.gain - target) * coef;
        self.gain
    }
}

pub struct DspChain {
    params: Arc<Params>,
    pitch: PhaseVocoder,
    eq: ParametricEq,
    formant: FormantShifter,
    radio: RadioFx,
    last: DspSnapshot,
}

impl DspChain {
    pub fn new(params: Arc<Params>) -> Self {
        Self {
            params,
            pitch: PhaseVocoder::new(SAMPLE_RATE),
            eq: ParametricEq::new(),
            formant: FormantShifter::new(),
            radio: RadioFx::new(),
            last: DspSnapshot::default(),
        }
    }

    pub fn process_block(&mut self, frames: &mut [f32]) {
        let snap = self.params.snapshot();
        if snap != self.last {
            self.eq.update(&snap);
            self.formant.update(snap.formant_shift);
            self.radio.update(&snap);
            self.pitch.set_semitones(snap.pitch_semitones);
            if snap.pitch_enabled != self.last.pitch_enabled {
                self.pitch.reset();
            }
            self.last = snap;
        }

        let pitch_on = snap.pitch_enabled;
        let formant_on = (snap.formant_shift - 1.0).abs() > 0.001;
        let gate = 10f32.powf(snap.noise_gate_db / 20.0).max(1e-6);
        for s in frames.iter_mut() {
            let mut x = *s;
            if snap.noise_gate_enabled {
                let level = (x.abs() / gate).clamp(0.0, 1.0);
                if level < 1.0 {
                    x *= level.sqrt();
                }
            }
            if pitch_on {
                x = self.pitch.process_sample(x);
            }
            if formant_on {
                x = self.formant.process(x);
            }
            x = self.eq.process(x);
            if snap.radio_enabled {
                x = self.radio.process(x);
            }
            *s = x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaking_zero_db_is_identity() {
        let mut bq = Biquad::peaking(48_000.0, 1000.0, 0.0, 1.0);
        for &s in &[0.1f32, -0.2, 0.3, -0.4, 0.5] {
            assert!((bq.process(s) - s).abs() < 1e-5);
        }
    }

    #[test]
    fn low_shelf_boosts_dc() {
        let mut bq = Biquad::low_shelf(48_000.0, 200.0, 12.0);
        let mut y = 0.0f32;
        for _ in 0..4096 {
            y = bq.process(1.0);
        }
        assert!((y - 3.98).abs() < 0.2, "DC gain was {y}");
    }

    fn steady_peak_hz(vocoder: &mut PhaseVocoder, freq: f32) -> f32 {
        let n = 65_536;
        let out: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                vocoder.process_sample((2.0 * PI * freq * t).sin())
            })
            .collect();
        let start = n - 2048;
        let fft = FftPlanner::<f32>::new().plan_fft_forward(2048);
        let mut buf: Vec<Complex32> = out[start..].iter().map(|&s| Complex32::new(s, 0.0)).collect();
        fft.process(&mut buf);
        let (peak_bin, _) = buf[..1024]
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, c)| (i, c.norm()))
            .fold((0usize, 0f32), |acc, cur| if cur.1 > acc.1 { cur } else { acc });
        peak_bin as f32 * SAMPLE_RATE as f32 / 2048.0
    }

    #[test]
    fn vocoder_unity_ratio_preserves_pitch() {
        let mut vocoder = PhaseVocoder::new(SAMPLE_RATE);
        let peak = steady_peak_hz(&mut vocoder, 440.0);
        assert!((peak - 446.0).abs() < 20.0, "peak at {peak} Hz");
    }

    #[test]
    fn vocoder_octave_up_shifts_pitch() {
        let mut vocoder = PhaseVocoder::new(SAMPLE_RATE);
        vocoder.set_semitones(12.0);
        let peak = steady_peak_hz(&mut vocoder, 440.0);
        assert!((peak - 891.0).abs() < 40.0, "peak at {peak} Hz");
    }

    #[test]
    fn duck_envelope_attacks_and_releases() {
        let mut env = DuckEnvelope::new();
        let snap = DspSnapshot {
            duck_atten_db: -12.0,
            ..DspSnapshot::default()
        };
        for _ in 0..40 {
            env.next_gain(true, &snap, 2400);
        }
        assert!(env.gain < 0.3, "ducked gain {}", env.gain);
        for _ in 0..40 {
            env.next_gain(false, &snap, 2400);
        }
        assert!(env.gain > 0.99, "released gain {}", env.gain);
    }
}
