//! Audio mixer, authored cues, and subtitle bus for POWDERBURN.
//!
//! Playback remains platform-adapter based, while gain, dialogue ducking, cue
//! metadata, and subtitles are owned and tested in-process.

#![forbid(unsafe_code)]
#![allow(clippy::float_arithmetic)]

pub mod cues;
pub mod mixer;
pub mod subtitles;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

pub use cues::Sfx;
pub use subtitles::Subtitle;

#[derive(Debug)]
struct Shared {
    music_gain: AtomicU8,
    sfx_gain: AtomicU8,
    dialogue_active: AtomicBool,
    subtitle: Mutex<Option<subtitles::ActiveSubtitle>>,
}

/// Non-blocking audio system with independent music/SFX gain.
#[derive(Debug)]
pub struct AudioSystem {
    _sfx_dir: PathBuf,
    tx: mpsc::Sender<Sfx>,
    shared: Arc<Shared>,
}

impl AudioSystem {
    pub fn new(asset_root: &Path) -> Self {
        let sfx_dir = asset_root.join("audio");
        let (tx, rx) = mpsc::channel::<Sfx>();
        let shared = Arc::new(Shared {
            music_gain: AtomicU8::new(70),
            sfx_gain: AtomicU8::new(80),
            dialogue_active: AtomicBool::new(false),
            subtitle: Mutex::new(None),
        });

        let worker_dir = sfx_dir.clone();
        let worker_shared = Arc::clone(&shared);
        thread::spawn(move || {
            while let Ok(sfx) = rx.recv() {
                let cue = sfx.cue();
                let configured = if cue.music {
                    worker_shared.music_gain.load(Ordering::Relaxed)
                } else {
                    worker_shared.sfx_gain.load(Ordering::Relaxed)
                };
                let gain = mixer::effective_gain_percent(
                    configured,
                    worker_shared.dialogue_active.load(Ordering::Relaxed),
                );
                if gain == 0 {
                    continue;
                }
                let source = worker_dir.join(cue.filename);
                let playback = if gain == 100 {
                    source
                } else {
                    match scaled_cache_file(&source, gain) {
                        Ok(path) => path,
                        Err(error) => {
                            eprintln!("{error}");
                            continue;
                        }
                    }
                };
                if std::process::Command::new("aplay")
                    .arg("-q")
                    .arg(&playback)
                    .spawn()
                    .is_err()
                {
                    let _ = std::process::Command::new("paplay").arg(&playback).spawn();
                }
            }
        });

        Self {
            _sfx_dir: sfx_dir,
            tx,
            shared,
        }
    }

    pub fn set_volumes(&self, music: f32, sfx: f32) {
        self.shared
            .music_gain
            .store(volume_percent(music), Ordering::Relaxed);
        self.shared
            .sfx_gain
            .store(volume_percent(sfx), Ordering::Relaxed);
    }

    pub fn set_dialogue_active(&self, active: bool) {
        self.shared.dialogue_active.store(active, Ordering::Relaxed);
    }

    pub fn play(&self, sfx: Sfx) {
        let cue = sfx.cue();
        if let Ok(mut active) = self.shared.subtitle.lock() {
            *active = Some(subtitles::ActiveSubtitle {
                subtitle: Subtitle {
                    speaker: cue.speaker,
                    text: cue.subtitle,
                },
                started: std::time::Instant::now(),
            });
        }
        let _ = self.tx.send(sfx);
    }

    pub fn play_many(&self, sfxs: &[Sfx]) {
        for &sfx in sfxs {
            self.play(sfx);
        }
    }

    pub fn current_subtitle(&self) -> Option<Subtitle> {
        let active = self.shared.subtitle.lock().ok()?;
        active
            .as_ref()
            .filter(|value| value.started.elapsed() <= Duration::from_secs(3))
            .map(|value| value.subtitle)
    }
}

fn volume_percent(value: f32) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn scaled_cache_file(source: &Path, gain: u8) -> Result<PathBuf, String> {
    let filename = source
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "E-AUDIO-PATH: invalid cue filename".to_string())?;
    let cache_dir = std::env::temp_dir().join("powderburn-audio-cache");
    std::fs::create_dir_all(&cache_dir).map_err(|error| format!("E-AUDIO-CACHE: {error}"))?;
    let output = cache_dir.join(format!("{filename}-{gain}.wav"));
    if output.exists() {
        return Ok(output);
    }
    let source_bytes = std::fs::read(source).map_err(|error| format!("E-AUDIO-READ: {error}"))?;
    let scaled = mixer::attenuate_pcm16_wav(&source_bytes, gain)?;
    std::fs::write(&output, scaled).map_err(|error| format!("E-AUDIO-CACHE: {error}"))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_has_a_speaker_labeled_subtitle() {
        for sfx in Sfx::ALL {
            let cue = sfx.cue();
            assert!(!cue.filename.is_empty());
            assert!(!cue.speaker.is_empty());
            assert!(!cue.subtitle.is_empty());
        }
    }

    #[test]
    fn subtitle_display_always_includes_speaker() {
        let cue = Sfx::RifleShot.cue();
        let subtitle = Subtitle {
            speaker: cue.speaker,
            text: cue.subtitle,
        };
        assert_eq!(subtitle.to_string(), "[Battlefield] Rifle shot");
    }
}
