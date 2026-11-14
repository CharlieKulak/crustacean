//! Sound effects, synthesized at start-up (ported from the Python game's audio.py).

use crate::pyrandom::PyRandom;
use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use std::collections::HashMap;

const RATE: u32 = 22050;

/// (frequency, seconds, wave, gain, slide Hz, delay)
type Tone = (f64, f64, &'static str, f64, f64, f64);

fn effects() -> Vec<(&'static str, Vec<Tone>)> {
    vec![
        // a bright little "pop" rising in pitch, with a soft sparkle on top
        ("eat", vec![(520.0, 0.07, "sine", 0.4, 520.0, 0.0), (1046.0, 0.07, "triangle", 0.12, 260.0, 0.03)]),
        ("turn", vec![(330.0, 0.025, "sine", 0.12, 60.0, 0.0)]),
        ("die", vec![(220.0, 0.18, "square", 0.22, -120.0, 0.0), (150.0, 0.35, "triangle", 0.35, -90.0, 0.12)]),
        ("start", vec![(440.0, 0.06, "sine", 0.3, 0.0, 0.0), (660.0, 0.08, "sine", 0.3, 0.0, 0.07)]),
        ("portal", vec![(300.0, 0.25, "sine", 0.3, 900.0, 0.0)]),
        ("push", vec![(140.0, 0.07, "triangle", 0.25, 0.0, 0.0)]),
        ("unlock", vec![(660.0, 0.07, "square", 0.2, 0.0, 0.0), (990.0, 0.1, "square", 0.2, 0.0, 0.07)]),
        ("shieldup", vec![(440.0, 0.12, "sine", 0.3, 440.0, 0.0)]),
        ("shield", vec![(200.0, 0.2, "triangle", 0.35, -100.0, 0.0)]),
        ("poison", vec![(180.0, 0.3, "saw", 0.25, -120.0, 0.0)]),
        ("dimension", vec![(700.0, 0.18, "sine", 0.25, -400.0, 0.0)]),
        ("arrow", vec![(600.0, 0.05, "triangle", 0.2, 0.0, 0.0)]),
        ("poof", vec![(400.0, 0.12, "noise", 0.15, 0.0, 0.0)]),
        ("win", [523.0, 659.0, 784.0, 1046.0].iter().enumerate()
            .map(|(i, &f)| (f, 0.15, "square", 0.2, 0.0, i as f64 * 0.12)).collect()),
        ("click", vec![(880.0, 0.02, "sine", 0.12, -200.0, 0.0)]),
        ("coin", vec![(988.0, 0.06, "square", 0.15, 0.0, 0.0), (1319.0, 0.18, "square", 0.15, 0.0, 0.06)]),
        ("tick", vec![(1500.0, 0.012, "square", 0.08, 0.0, 0.0)]),
        ("reveal", [392.0, 523.0, 659.0, 784.0, 1046.0, 1318.0].iter().enumerate()
            .map(|(i, &f)| (f, 0.2, "triangle", 0.22, 0.0, i as f64 * 0.07)).collect()),
    ]
}

fn wave(kind: &str, phase: f64, rng: &mut PyRandom) -> f64 {
    let p = phase.rem_euclid(1.0);
    match kind {
        "square" => if p < 0.5 { 1.0 } else { -1.0 },
        "triangle" => if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p },
        "saw" => 2.0 * p - 1.0,
        "noise" => rng.random() * 2.0 - 1.0,
        _ => (std::f64::consts::TAU * p).sin(),
    }
}

fn synth_samples(tones: &[Tone]) -> Vec<f32> {
    let total = tones.iter().map(|t| ((t.5 + t.1) * RATE as f64) as usize).max().unwrap_or(0) + 1;
    let mut buf = vec![0.0f64; total];
    let mut rng = PyRandom::new(7);
    for &(freq, dur, kind, gain, slide, delay) in tones {
        let start = (delay * RATE as f64) as usize;
        let n = (dur * RATE as f64) as usize;
        let mut phase = 0.0;
        let mut lp = 0.0;
        let grain = ((RATE as f64 * 0.004) as usize).max(1);
        let mut level = 1.0;
        for i in 0..n {
            let t = i as f64 / n as f64;
            let env = (1.0 - t).powi(2) * (i as f64 / 40.0).min(1.0);
            let v = if kind == "crunch" {
                // low-passed noise chopped into grains of random loudness, like a bite
                if i % grain == 0 {
                    level = 0.15 + rng.random() * 0.85;
                }
                let f = freq + slide * t;
                let a = (std::f64::consts::TAU * f / RATE as f64).min(1.0);
                lp += a * (rng.random() * 2.0 - 1.0 - lp);
                lp * level * 2.2
            } else {
                let f = if slide != 0.0 { freq * ((freq + slide) / freq).powf(t) } else { freq };
                phase += f / RATE as f64;
                wave(kind, phase, &mut rng)
            };
            buf[start + i] += v * gain * env;
        }
    }
    buf.into_iter().map(|v| v as f32).collect()
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

pub struct Sfx {
    sounds: HashMap<&'static str, Sound>,
    /// On Linux: sound through PulseAudio / PipeWire (when it's there).
    #[cfg(target_os = "linux")]
    pulse: Option<pulse::Pulse>,
    pub volume: f32,
}

impl Sfx {
    pub async fn load() -> Sfx {
        let mut sounds = HashMap::new();
        #[cfg(target_os = "linux")]
        {
            // macroquad's sound only speaks ALSA, which (without the pipewire-alsa
            // package) can't reach the sound card on PipeWire systems. Talk to the
            // sound server directly instead, and only fall back to ALSA without one.
            let clips: Vec<(&'static str, Vec<f32>)> = effects().into_iter().map(|(n, t)| (n, synth_samples(&t))).collect();
            if let Some(p) = pulse::Pulse::start(clips) {
                return Sfx { sounds, pulse: Some(p), volume: 0.8 };
            }
        }
        for (name, tones) in effects() {
            if let Ok(s) = load_sound_from_bytes(&wav(&synth_samples(&tones))).await {
                sounds.insert(name, s);
            }
        }
        Sfx {
            sounds,
            #[cfg(target_os = "linux")]
            pulse: None,
            volume: 0.8,
        }
    }

    pub fn play(&self, name: &str) {
        if self.volume <= 0.0 {
            return;
        }
        let v = if name == "eat" { 0.8 } else { 1.0 };
        #[cfg(target_os = "linux")]
        if let Some(p) = &self.pulse {
            p.play(name, self.volume * v);
            return;
        }
        if let Some(s) = self.sounds.get(name) {
            play_sound(s, PlaySoundParams { looped: false, volume: self.volume * v });
        }
    }
}

#[cfg(target_os = "linux")]
mod pulse {
    //! A tiny mixer that plays through libpulse-simple (PulseAudio, or PipeWire's
    //! PulseAudio server), loaded at run time so the game still starts without it.

    use libloading::{Library, Symbol};
    use std::collections::HashMap;
    use std::ffi::{c_char, c_int, c_void};
    use std::sync::mpsc::{channel, Sender};

    const RATE: u32 = super::RATE;
    const CHUNK: usize = 256; // samples per write (about 12 ms)

    #[repr(C)]
    struct SampleSpec {
        format: c_int, // PA_SAMPLE_S16LE = 3
        rate: u32,
        channels: u8,
    }

    #[repr(C)]
    struct BufferAttr {
        maxlength: u32,
        tlength: u32,
        prebuf: u32,
        minreq: u32,
        fragsize: u32,
    }

    type New = unsafe extern "C" fn(*const c_char, *const c_char, c_int, *const c_char, *const c_char,
                                    *const SampleSpec, *const c_void, *const BufferAttr, *mut c_int) -> *mut c_void;
    type Write = unsafe extern "C" fn(*mut c_void, *const c_void, usize, *mut c_int) -> c_int;

    pub struct Pulse {
        tx: Sender<(usize, f32)>,
        names: HashMap<&'static str, usize>,
    }

    impl Pulse {
        pub fn start(clips: Vec<(&'static str, Vec<f32>)>) -> Option<Pulse> {
            let lib = match unsafe { Library::new("libpulse-simple.so.0") } {
                Ok(l) => l,
                Err(e) => {
                    if std::env::var("OPENSNAKE_DEBUG").is_ok() {
                        eprintln!("no PulseAudio sound: {e}");
                    }
                    return None;
                }
            };
            let spec = SampleSpec { format: 3, rate: RATE, channels: 1 };
            // keep about 40 ms queued: enough not to stutter, little enough to feel instant
            let target = (RATE as usize * 2 * 40 / 1000) as u32;
            let attr = BufferAttr { maxlength: u32::MAX, tlength: target, prebuf: u32::MAX, minreq: u32::MAX, fragsize: u32::MAX };
            let handle = unsafe {
                let new: Symbol<New> = lib.get(b"pa_simple_new\0").ok()?;
                let mut err = 0;
                new(std::ptr::null(), c"OpenSnake".as_ptr(), 1, std::ptr::null(), c"Sound effects".as_ptr(), &spec,
                    std::ptr::null(), &attr, &mut err)
            };
            if handle.is_null() {
                if std::env::var("OPENSNAKE_DEBUG").is_ok() {
                    eprintln!("no PulseAudio sound: pa_simple_new failed");
                }
                return None;
            }
            let handle = handle as usize; // (moved into the mixer thread)
            let names: HashMap<&'static str, usize> = clips.iter().enumerate().map(|(i, (n, _))| (*n, i)).collect();
            let samples: Vec<Vec<f32>> = clips.into_iter().map(|(_, s)| s).collect();
            let (tx, rx) = channel::<(usize, f32)>();
            std::thread::spawn(move || {
                let write: Symbol<Write> = match unsafe { lib.get(b"pa_simple_write\0") } {
                    Ok(w) => w,
                    Err(_) => return,
                };
                let mut voices: Vec<(usize, usize, f32)> = Vec::new(); // (clip, position, volume)
                let mut buf = [0i16; CHUNK];
                let mut mix = [0f32; CHUNK];
                loop {
                    while let Ok((clip, vol)) = rx.try_recv() {
                        voices.push((clip, 0, vol));
                    }
                    mix.fill(0.0);
                    for (clip, pos, vol) in voices.iter_mut() {
                        let s = &samples[*clip];
                        for m in mix.iter_mut() {
                            if *pos >= s.len() {
                                break;
                            }
                            *m += s[*pos] * *vol;
                            *pos += 1;
                        }
                    }
                    voices.retain(|(clip, pos, _)| *pos < samples[*clip].len());
                    for (b, m) in buf.iter_mut().zip(mix.iter()) {
                        *b = (m.clamp(-1.0, 1.0) * 32767.0) as i16;
                    }
                    let mut err = 0;
                    // (blocks until there's room, which paces this loop)
                    if unsafe { write(handle as *mut c_void, buf.as_ptr() as *const c_void, CHUNK * 2, &mut err) } < 0 {
                        return; // the sound server went away: just go quiet
                    }
                }
            });
            if std::env::var("OPENSNAKE_DEBUG").is_ok() {
                eprintln!("sound: PulseAudio");
            }
            Some(Pulse { tx, names })
        }

        pub fn play(&self, name: &str, volume: f32) {
            if let Some(&i) = self.names.get(name) {
                let _ = self.tx.send((i, volume));
            }
        }
    }
}
