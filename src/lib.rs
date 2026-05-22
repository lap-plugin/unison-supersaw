use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;

const MAX_UNISON: usize = 32;
const MAX_POLY: usize = 32;

#[inline(always)]
fn poly_blep(mut t: f32, dt: f32) -> f32 {
    if t < dt {
        t /= dt;
        return t + t - t * t - 1.0;
    } else if t > 1.0 - dt {
        t = (t - 1.0) / dt;
        return t * t + t + t + 1.0;
    }
    0.0
}

// ══════════════════════════════════════════════════════════════════════════════
// AMD ENVELOPE (Attack, Multiply, Decay)
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq)]
enum EnvState {
    Idle,
    Attack,
    Multiply,
    Decay,
}

#[derive(Clone, Copy)]
struct Envelope {
    state: EnvState,
    level: f32,
    time: f32,
    attack_time: f32,
    multiply_time: f32,
    decay_time: f32,
    sample_rate: f32,
}

impl Envelope {
    fn new() -> Self {
        Self {
            state: EnvState::Idle,
            level: 0.0,
            time: 0.0,
            attack_time: 0.01,
            multiply_time: 0.05,
            decay_time: 0.3,
            sample_rate: 44100.0,
        }
    }

    fn trigger(&mut self, attack_ms: f32, multiply_ms: f32, decay_ms: f32, sr: f32) {
        self.state = EnvState::Attack;
        self.time = 0.0;
        self.attack_time = attack_ms / 1000.0;
        self.multiply_time = multiply_ms / 1000.0;
        self.decay_time = decay_ms / 1000.0;
        self.sample_rate = sr;
    }

    fn is_active(&self) -> bool {
        self.state != EnvState::Idle
    }

    #[inline(always)]
    fn process(&mut self) -> f32 {
        let dt = 1.0 / self.sample_rate;
        self.time += dt;

        match self.state {
            EnvState::Idle => 0.0,

            EnvState::Attack => {
                if self.time >= self.attack_time {
                    self.state = EnvState::Multiply;
                    self.time = 0.0;
                    self.level = 1.0;
                    1.0
                } else {
                    let t = self.time / self.attack_time;
                    self.level = t * t; // Quadratic
                    self.level
                }
            }

            EnvState::Multiply => {
                if self.time >= self.multiply_time {
                    self.state = EnvState::Decay;
                    self.time = 0.0;
                    self.level = 1.0;
                    1.0
                } else {
                    // Hold at multiply
                    self.level = 1.0;
                    1.0
                }
            }

            EnvState::Decay => {
                if self.time >= self.decay_time {
                    self.state = EnvState::Idle;
                    self.level = 0.0;
                    0.0
                } else {
                    let t = self.time / self.decay_time;
                    // Exponential decay
                    let curve = (1.0 - t) * (1.0 - t);
                    self.level = curve;

                    // Early cutoff
                    if self.level < 1e-4 {
                        self.state = EnvState::Idle;
                        0.0
                    } else {
                        self.level
                    }
                }
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// OSCILLATOR LAYER (Unison Hypersaw)
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct OscLayer {
    base_freq: f32,
    phases: [f32; MAX_UNISON],
}

impl OscLayer {
    fn new() -> Self {
        Self {
            base_freq: 0.0,
            phases: [0.0; MAX_UNISON],
        }
    }

    fn trigger(&mut self, freq: f32) {
        self.base_freq = freq;

        // Random phases
        for i in 0..MAX_UNISON {
            self.phases[i] = (i as f32 * 0.6180339).fract();
        }
    }

    #[inline(always)]
    fn process_sample(
        &mut self,
        sample_rate: f32,
        unison_count: usize,
        detune: f32,
        spread: f32,
    ) -> (f32, f32) {
        let mut left = 0.0f32;
        let mut right = 0.0f32;

        for i in 0..unison_count.min(MAX_UNISON) {
            // RANDOM DETUNE (víťaz!)
            let voice_detune = if unison_count > 1 {
                let detune_range = detune * 0.05;
                let pseudo_rand = ((i as f32 * 12.9898 + 78.233).sin() * 43758.5453).fract();
                (pseudo_rand * 2.0 - 1.0) * detune_range
            } else {
                0.0
            };

            let detuned_freq = self.base_freq * (voice_detune).exp2();
            let phase_inc = detuned_freq / sample_rate;

            // SAW + poly-BLEP
            let phase = self.phases[i];
            let mut sample = (phase * 2.0) - 1.0;
            sample -= poly_blep(phase, phase_inc);

            // RANDOM SPREAD (víťaz!)
            let pan = if unison_count > 1 {
                let pseudo_rand = ((i as f32 * 7.1234 + 31.415).sin() * 21983.7412).fract();
                let raw_pan = pseudo_rand * 2.0 - 1.0;
                raw_pan * spread
            } else {
                0.0
            };

            // Constant power panning
            let pan_angle = (pan * 0.5 + 0.5) * std::f32::consts::PI * 0.5;
            let (sin, cos) = pan_angle.sin_cos();

            left += sample * cos;
            right += sample * sin;

            // Update phase
            self.phases[i] += phase_inc;
            if self.phases[i] >= 1.0 {
                self.phases[i] -= 1.0;
            }
        }

        // Unison normalization
        let norm = 1.0 / (unison_count as f32).sqrt();

        (left * norm, right * norm)
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// POLYPHONIC VOICE (4× Hypersaw layers + ADSR)
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct PolyVoice {
    note: u8,
    velocity: f32,
    envelope: Envelope,

    // 4× Hypersaw layers
    hypersaw1: OscLayer,
    hypersaw2: OscLayer,
    hypersaw3: OscLayer,
    hypersaw4: OscLayer,
}

impl PolyVoice {
    fn new() -> Self {
        Self {
            note: 0,
            velocity: 0.0,
            envelope: Envelope::new(),
            hypersaw1: OscLayer::new(),
            hypersaw2: OscLayer::new(),
            hypersaw3: OscLayer::new(),
            hypersaw4: OscLayer::new(),
        }
    }

    fn trigger(
        &mut self,
        note: u8,
        velocity: f32,
        attack: f32,
        multiply: f32,
        decay: f32,
        sr: f32,
    ) {
        self.note = note;
        self.velocity = velocity;
        self.envelope.trigger(attack, multiply, decay, sr);

        let freq = util::midi_note_to_freq(note);
        self.hypersaw1.trigger(freq);
        self.hypersaw2.trigger(freq);
        self.hypersaw3.trigger(freq);
        self.hypersaw4.trigger(freq);
    }

    fn is_active(&self) -> bool {
        self.envelope.is_active()
    }

    fn process(&mut self, sr: f32, unison_count: usize, detune: f32, spread: f32) -> (f32, f32) {
        let env = self.envelope.process();

        if env < 1e-6 {
            return (0.0, 0.0);
        }

        // Process 4× Hypersaw layers
        let (l1, r1) = self
            .hypersaw1
            .process_sample(sr, unison_count, detune, spread);
        let (l2, r2) = self
            .hypersaw2
            .process_sample(sr, unison_count, detune, spread);
        let (l3, r3) = self
            .hypersaw3
            .process_sample(sr, unison_count, detune, spread);
        let (l4, r4) = self
            .hypersaw4
            .process_sample(sr, unison_count, detune, spread);

        // Mix 2 layers
        let mut left = l1 + l2 + l3 + l4;
        let mut right = r1 + r2 + r3 + r4;

        // Normalize 4 oscillators (1/sqrt(2) = 0.5)
        let osc_norm = 0.5;
        left *= osc_norm;
        right *= osc_norm;

        // Apply envelope and velocity
        left *= env * self.velocity;
        right *= env * self.velocity;

        (left, right)
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// MAIN PLUGIN
// ══════════════════════════════════════════════════════════════════════════════

struct HypersawSimple {
    params: Arc<HypersawParams>,
    poly_voices: Vec<PolyVoice>,
    sample_rate: f32,
}

#[derive(Params)]
struct HypersawParams {
    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "attack"]
    pub attack: FloatParam,

    #[id = "multiply"]
    pub multiply: FloatParam,

    #[id = "decay"]
    pub decay: FloatParam,

    #[id = "unison_voices"]
    pub unison_voices: IntParam,

    #[id = "detune"]
    pub detune: FloatParam,

    #[id = "spread"]
    pub spread: FloatParam,
}

impl Default for HypersawParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(-12.0), // -12dB (pevná norm = stabilná hlasitosť)
                FloatRange::Skewed {
                    min: util::db_to_gain(-36.0),
                    max: util::db_to_gain(0.0),
                    factor: FloatRange::gain_skew_factor(-36.0, 0.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            attack: FloatParam::new(
                "Attack",
                10.0, // 10ms default
                FloatRange::Skewed {
                    min: 1.0,
                    max: 80.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            multiply: FloatParam::new(
                "Multiply",
                40.0, // 40ms default (hold at multiply)
                FloatRange::Skewed {
                    min: 5.0,
                    max: 150.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            decay: FloatParam::new(
                "Decay",
                300.0, // 300ms default
                FloatRange::Skewed {
                    min: 10.0,
                    max: 600.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" ms"),

            unison_voices: IntParam::new(
                "Unison Voices",
                10, // 10 voices default
                IntRange::Linear { min: 1, max: 32 },
            ),

            detune: FloatParam::new(
                "Unison Detune",
                0.3, // 30% (wide supersaw sound)
                FloatRange::Linear { min: 0.2, max: 0.8 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),

            spread: FloatParam::new(
                "Unison Spread",
                0.7, // 70% (wider than Hypersaw 70%)
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
        }
    }
}

impl Default for HypersawSimple {
    fn default() -> Self {
        Self {
            params: Arc::new(HypersawParams::default()),
            poly_voices: (0..MAX_POLY).map(|_| PolyVoice::new()).collect(),
            sample_rate: 44100.0,
        }
    }
}

impl Plugin for HypersawSimple {
    const NAME: &'static str = "LAP Unison Supersaw";
    const VENDOR: &'static str = "lap-plugin";
    const URL: &'static str = "";
    const EMAIL: &'static str = "lap-plugin@technologies.com";
    const VERSION: &'static str = "1.0.0";

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        true
    }

    fn reset(&mut self) {
        self.poly_voices = (0..MAX_POLY).map(|_| PolyVoice::new()).collect();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            // MIDI handling
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => {
                        // Find free voice or steal oldest
                        let voice_idx = self
                            .poly_voices
                            .iter()
                            .position(|v| !v.is_active())
                            .or_else(|| Some(0)); // Steal first if all busy

                        if let Some(idx) = voice_idx {
                            self.poly_voices[idx].trigger(
                                note,
                                velocity,
                                self.params.attack.value(),
                                self.params.multiply.value(),
                                self.params.decay.value(),
                                self.sample_rate,
                            );
                        }
                    }
                    // AMD envelope = one-shot, no NoteOff handling needed
                    _ => {}
                }
                next_event = context.next_event();
            }

            // Get parameters
            let gain = self.params.gain.smoothed.next();
            let unison_count = self.params.unison_voices.value() as usize;
            let detune = self.params.detune.smoothed.next();
            let spread = self.params.spread.smoothed.next();

            // Sum all active voices
            let mut left_out = 0.0f32;
            let mut right_out = 0.0f32;

            for voice in &mut self.poly_voices {
                if voice.is_active() {
                    let (l, r) = voice.process(self.sample_rate, unison_count, detune, spread);
                    left_out += l;
                    right_out += r;
                }
            }

            // PEVNÁ normalizácia pre max 32 poly voices
            // Stabilná hlasitosť bez ohľadu na počet aktívnych voices!
            let norm = 1.0 / 32.0f32.sqrt(); // = ~0.177
            left_out *= norm;
            right_out *= norm;

            // Soft clipping (tanh)
            left_out = (left_out * 1.2).tanh();
            right_out = (right_out * 1.2).tanh();

            // Apply gain
            let mut output = channel_samples.into_iter();
            if let Some(left) = output.next() {
                *left = left_out * gain;
            }
            if let Some(right) = output.next() {
                *right = right_out * gain;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for HypersawSimple {
    const CLAP_ID: &'static str = "com.bda.lap-supersaw";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Polyphonic Unison Supersaw Oscillator)");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for HypersawSimple {
    const VST3_CLASS_ID: [u8; 16] = *b"UniHypersawAMDXX";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(HypersawSimple);
nih_export_vst3!(HypersawSimple);
