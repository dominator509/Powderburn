//! Audio mixer, authored cues, and subtitle bus for POWDERBURN.
//!
//! Playback remains platform-adapter based, while gain, dialogue ducking, cue
//! metadata, and subtitles are owned and tested in-process.

#![forbid(unsafe_code)]
#![allow(clippy::float_arithmetic)]

pub mod cues;
pub mod mixer;
pub mod subtitles;

use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
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
    running: AtomicBool,
    score_revision: AtomicU64,
    score_filename: Mutex<String>,
    subtitle: Mutex<Option<subtitles::ActiveSubtitle>>,
}

#[derive(Debug)]
struct PlaybackRequest {
    filename: String,
    music: bool,
    dialogue: bool,
}

/// Non-blocking audio system with independent music/SFX gain.
#[derive(Debug)]
pub struct AudioSystem {
    _sfx_dir: PathBuf,
    tx: mpsc::Sender<PlaybackRequest>,
    shared: Arc<Shared>,
}

impl AudioSystem {
    pub fn new(asset_root: &Path) -> Self {
        let sfx_dir = asset_root.join("audio");
        let (tx, rx) = mpsc::channel::<PlaybackRequest>();
        let shared = Arc::new(Shared {
            music_gain: AtomicU8::new(70),
            sfx_gain: AtomicU8::new(80),
            dialogue_active: AtomicBool::new(false),
            running: AtomicBool::new(true),
            score_revision: AtomicU64::new(0),
            score_filename: Mutex::new(Sfx::FrontierTheme.filename().to_string()),
            subtitle: Mutex::new(None),
        });

        let worker_dir = sfx_dir.clone();
        let worker_shared = Arc::clone(&shared);
        thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                let configured = if request.music {
                    worker_shared.music_gain.load(Ordering::Relaxed)
                } else {
                    worker_shared.sfx_gain.load(Ordering::Relaxed)
                };
                let gain = if request.dialogue {
                    configured
                } else {
                    mixer::effective_gain_percent(
                        configured,
                        worker_shared.dialogue_active.load(Ordering::Relaxed),
                    )
                };
                if gain == 0 {
                    if request.dialogue {
                        worker_shared
                            .dialogue_active
                            .store(false, Ordering::Relaxed);
                    }
                    continue;
                }
                let source = worker_dir.join(&request.filename);
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
                let child = std::process::Command::new("aplay")
                    .arg("-q")
                    .arg(&playback)
                    .spawn()
                    .or_else(|_| std::process::Command::new("paplay").arg(&playback).spawn());
                if request.dialogue {
                    if let Ok(mut child) = child {
                        let _ = child.wait();
                    }
                    worker_shared
                        .dialogue_active
                        .store(false, Ordering::Relaxed);
                }
            }
        });

        // Keep the frontier score alive for the full session.  The original
        // client dispatched it once at startup, leaving the game silent after
        // the first short WAV ended.
        let music_dir = sfx_dir.clone();
        let music_shared = Arc::clone(&shared);
        thread::spawn(move || {
            while music_shared.running.load(Ordering::Relaxed) {
                let revision = music_shared.score_revision.load(Ordering::Relaxed);
                let filename = music_shared.score_filename.lock().map_or_else(
                    |_| Sfx::FrontierTheme.filename().to_string(),
                    |value| value.clone(),
                );
                let source = music_dir.join(filename);
                let dialogue_active = music_shared.dialogue_active.load(Ordering::Relaxed);
                let gain = mixer::effective_gain_percent(
                    music_shared.music_gain.load(Ordering::Relaxed),
                    dialogue_active,
                );
                if gain == 0 {
                    thread::sleep(Duration::from_millis(250));
                    continue;
                }
                let playback = if gain == 100 {
                    source.clone()
                } else {
                    match scaled_cache_file(&source, gain) {
                        Ok(path) => path,
                        Err(error) => {
                            eprintln!("{error}");
                            thread::sleep(Duration::from_secs(2));
                            continue;
                        }
                    }
                };
                let child = std::process::Command::new("paplay")
                    .arg(&playback)
                    .spawn()
                    .or_else(|_| {
                        std::process::Command::new("aplay")
                            .arg("-q")
                            .arg(&playback)
                            .spawn()
                    });
                let Ok(mut child) = child else {
                    eprintln!("E-AUDIO-PLAYER: install paplay or aplay to hear game audio");
                    thread::sleep(Duration::from_secs(2));
                    continue;
                };
                loop {
                    if !music_shared.running.load(Ordering::Relaxed) {
                        let _ = child.kill();
                        return;
                    }
                    if music_shared.score_revision.load(Ordering::Relaxed) != revision
                        || music_shared.dialogue_active.load(Ordering::Relaxed) != dialogue_active
                    {
                        let _ = child.kill();
                        break;
                    }
                    match child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) => thread::sleep(Duration::from_millis(100)),
                        Err(_) => break,
                    }
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

    /// Switch the looping mood score. Unsafe paths are rejected.
    pub fn set_score(&self, filename: &str) -> bool {
        if !safe_audio_path(filename) {
            return false;
        }
        let Ok(mut current) = self.shared.score_filename.lock() else {
            return false;
        };
        if current.as_str() == filename {
            return true;
        }
        *current = filename.to_string();
        self.shared.score_revision.fetch_add(1, Ordering::Relaxed);
        true
    }

    pub fn play(&self, sfx: Sfx) {
        let cue = sfx.cue();
        self.set_subtitle(cue.speaker, cue.subtitle);
        let _ = self.tx.send(PlaybackRequest {
            filename: cue.filename.to_string(),
            music: cue.music,
            dialogue: false,
        });
    }

    /// Play one authored dialogue WAV and duck the score until it finishes.
    pub fn play_dialogue(&self, filename: &str, speaker: &str, subtitle: &str) -> bool {
        let relative = format!("dialogue/{filename}");
        if !safe_audio_path(&relative) {
            return false;
        }
        self.set_subtitle(speaker, subtitle);
        self.shared.dialogue_active.store(true, Ordering::Relaxed);
        self.tx
            .send(PlaybackRequest {
                filename: relative,
                music: false,
                dialogue: true,
            })
            .is_ok()
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
            .map(|value| value.subtitle.clone())
    }

    fn set_subtitle(&self, speaker: &str, text: &str) {
        if let Ok(mut active) = self.shared.subtitle.lock() {
            *active = Some(subtitles::ActiveSubtitle {
                subtitle: Subtitle {
                    speaker: speaker.to_string(),
                    text: text.to_string(),
                },
                started: std::time::Instant::now(),
            });
        }
    }
}

impl Drop for AudioSystem {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::Relaxed);
    }
}

fn volume_percent(value: f32) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    (value.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn safe_audio_path(filename: &str) -> bool {
    let path = Path::new(filename);
    path.extension().and_then(|value| value.to_str()) == Some("wav")
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
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
        let source_len = std::fs::metadata(source)
            .map(|metadata| metadata.len())
            .ok();
        let cached_len = std::fs::metadata(&output)
            .map(|metadata| metadata.len())
            .ok();
        if source_len == cached_len {
            return Ok(output);
        }
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
            speaker: cue.speaker.to_string(),
            text: cue.subtitle.to_string(),
        };
        assert_eq!(subtitle.to_string(), "[Battlefield] Rifle shot");
    }

    #[test]
    fn gendered_combat_cues_are_distinct_and_catalogued() {
        assert_ne!(Sfx::DamageMale.filename(), Sfx::DamageFemale.filename());
        assert_ne!(Sfx::DeathMale.filename(), Sfx::DeathFemale.filename());
        assert!(Sfx::ALL.contains(&Sfx::DamageMale));
        assert!(Sfx::ALL.contains(&Sfx::DamageFemale));
        assert!(Sfx::ALL.contains(&Sfx::DeathMale));
        assert!(Sfx::ALL.contains(&Sfx::DeathFemale));
    }
}
