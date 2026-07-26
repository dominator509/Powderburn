//! Audio playback for POWDERBURN using system tools (aplay/paplay).
//!
//! No Rust audio dependencies. WAV files are played via subprocess.
//! See ARCHITECTURE.md for this crate's place in the import law.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

/// A sound effect identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    PistolShot,
    RifleShot,
    Hit,
    Miss,
    Click,
    Select,
    Victory,
    Death,
    Move,
    Reload,
}

impl Sfx {
    /// Return the WAV filename for this sound effect.
    pub fn filename(&self) -> &'static str {
        match self {
            Sfx::PistolShot => "pistol_shot.wav",
            Sfx::RifleShot => "rifle_shot.wav",
            Sfx::Hit => "hit.wav",
            Sfx::Miss => "miss.wav",
            Sfx::Click => "click.wav",
            Sfx::Select => "select.wav",
            Sfx::Victory => "victory.wav",
            Sfx::Death => "death.wav",
            Sfx::Move => "move.wav",
            Sfx::Reload => "reload.wav",
        }
    }
}

/// The audio system. Spawns a background thread that plays WAV files
/// via `aplay` (or `paplay` as fallback).
#[derive(Debug)]
pub struct AudioSystem {
    sfx_dir: PathBuf,
    tx: mpsc::Sender<Sfx>,
}

impl AudioSystem {
    /// Create a new audio system with a background playback thread.
    ///
    /// `asset_root` should point to the `assets/` directory; WAV files
    /// are expected in `assets/audio/`.
    pub fn new(asset_root: &PathBuf) -> Self {
        let sfx_dir = asset_root.join("audio");
        let (tx, rx) = mpsc::channel::<Sfx>();

        // Spawn background playback thread
        let sfx_dir_clone = sfx_dir.clone();
        thread::spawn(move || {
            while let Ok(sfx) = rx.recv() {
                let filepath = sfx_dir_clone.join(sfx.filename());
                let path_str = filepath.to_string_lossy().to_string();

                // Try aplay first (ALSA), then paplay (PulseAudio), then silently fail
                let result = std::process::Command::new("aplay")
                    .arg("-q")
                    .arg(&path_str)
                    .spawn();

                if result.is_err() {
                    // aplay not found, try paplay
                    let _ = std::process::Command::new("paplay").arg(&path_str).spawn();
                }
            }
        });

        AudioSystem { sfx_dir, tx }
    }

    /// Play a sound effect (non-blocking). Returns immediately; playback
    /// happens on the background thread.
    pub fn play(&self, sfx: Sfx) {
        let _ = self.tx.send(sfx);
    }

    /// Convenience: play multiple sound effects at once.
    pub fn play_many(&self, sfxs: &[Sfx]) {
        for &sfx in sfxs {
            let _ = self.tx.send(sfx);
        }
    }
}
