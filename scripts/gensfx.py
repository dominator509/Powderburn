#!/usr/bin/env python3
"""Generate procedural WAV sound effects for POWDERBURN.

Writes 44100 Hz 16-bit mono WAV files to ../assets/audio/.
"""

import struct
import math
import random
import os

SAMPLE_RATE = 44100
OUT_DIR = os.path.join(os.path.dirname(__file__), "..", "assets", "audio")


def write_wav(path, samples, sample_rate=SAMPLE_RATE):
    """Write 16-bit mono WAV file from float samples in [-1, 1]."""
    num_samples = len(samples)
    data = b""
    for s in samples:
        s = max(-32768, min(32767, int(s * 32767)))
        data += struct.pack("<h", s)

    with open(path, "wb") as f:
        # RIFF header
        f.write(b"RIFF")
        f.write(struct.pack("<I", 36 + len(data)))
        f.write(b"WAVE")
        # fmt chunk
        f.write(b"fmt ")
        f.write(struct.pack("<I", 16))  # chunk size
        f.write(struct.pack("<H", 1))  # PCM
        f.write(struct.pack("<H", 1))  # mono
        f.write(struct.pack("<I", sample_rate))
        f.write(struct.pack("<I", sample_rate * 2))  # byte rate
        f.write(struct.pack("<H", 2))  # block align
        f.write(struct.pack("<H", 16))  # bits per sample
        # data chunk
        f.write(b"data")
        f.write(struct.pack("<I", len(data)))
        f.write(data)


def envelope_adsr(t, attack, decay, sustain, release, duration):
    """Simple ADSR envelope returning gain at time t (0..duration)."""
    if t < attack:
        return t / attack if attack > 0 else 1.0
    if t < attack + decay:
        return 1.0 - (1.0 - sustain) * (t - attack) / decay
    if t < duration - release:
        return sustain
    return sustain * (1.0 - (t - (duration - release)) / release)


def pistol_shot(duration=0.3, sample_rate=SAMPLE_RATE):
    """Short gunshot — white noise burst + low sine decay."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        noise = random.uniform(-1, 1)
        envelope = math.exp(-t * 15) * (1 - math.exp(-t * 200))
        tone = math.sin(2 * math.pi * 150 * t) * 0.3
        samples.append((noise * 0.7 + tone * 0.3) * envelope)
    return samples


def rifle_shot(duration=0.4, sample_rate=SAMPLE_RATE):
    """Deeper gunshot — lower freq noise + sub-bass thump."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        noise = random.uniform(-1, 1)
        envelope = math.exp(-t * 10) * (1 - math.exp(-t * 150))
        tone = math.sin(2 * math.pi * 80 * t) * 0.4
        sub = math.sin(2 * math.pi * 40 * t) * 0.2 * math.exp(-t * 20)
        samples.append((noise * 0.6 + tone * 0.3 + sub * 0.1) * envelope)
    return samples


def hit(duration=0.15, sample_rate=SAMPLE_RATE):
    """Impact thud — low frequency tone with sharp attack."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        envelope = math.exp(-t * 30) * (1 - math.exp(-t * 300))
        tone = math.sin(2 * math.pi * 120 * t) * 0.6
        noise = random.uniform(-1, 1) * 0.2
        samples.append((tone + noise) * envelope)
    return samples


def miss(duration=0.2, sample_rate=SAMPLE_RATE):
    """Bullet whiz — high frequency sweep."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        freq = 800 + 3000 * t  # sweep up
        envelope = math.sin(math.pi * t / duration)  # shaped burst
        tone = math.sin(2 * math.pi * freq * t) * 0.5
        samples.append(tone * envelope)
    return samples


def click(duration=0.05, sample_rate=SAMPLE_RATE):
    """UI click — short 1000Hz sine ping."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        envelope = math.exp(-t * 100)
        tone = math.sin(2 * math.pi * 1000 * t) * 0.5
        samples.append(tone * envelope)
    return samples


def select(duration=0.1, sample_rate=SAMPLE_RATE):
    """Actor selected — rising tone."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        freq = 400 + 600 * (t / duration)  # 400 → 1000 Hz
        envelope = math.sin(math.pi * t / duration)
        tone = math.sin(2 * math.pi * freq * t) * 0.4
        samples.append(tone * envelope)
    return samples


def victory(duration=0.5, sample_rate=SAMPLE_RATE):
    """Victory fanfare — major chord arpeggio (C-E-G-C)."""
    n = int(duration * sample_rate)
    notes = [261.63, 329.63, 392.00, 523.25]  # C4, E4, G4, C5
    samples = []
    for i in range(n):
        t = i / sample_rate
        envelope = math.sin(math.pi * t / duration)
        chord = 0.0
        for j, freq in enumerate(notes):
            phase_offset = j * 2.0 * math.pi / len(notes)
            chord += math.sin(2 * math.pi * freq * t + phase_offset) * 0.2
        samples.append(chord * envelope)
    return samples


def death(duration=0.3, sample_rate=SAMPLE_RATE):
    """Death sound — descending tone."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        freq = 400 - 300 * (t / duration)  # 400 → 100 Hz
        envelope = math.sin(math.pi * t / duration)
        tone = math.sin(2 * math.pi * freq * t) * 0.4
        samples.append(tone * envelope)
    return samples


def move_sound(duration=0.1, sample_rate=SAMPLE_RATE):
    """Footstep — low thud."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        envelope = math.exp(-t * 40) * (1 - math.exp(-t * 300))
        tone = math.sin(2 * math.pi * 60 * t) * 0.5
        noise = random.uniform(-1, 1) * 0.15
        samples.append((tone + noise) * envelope)
    return samples


def reload(duration=0.15, sample_rate=SAMPLE_RATE):
    """Reload click — metallic click."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        # Two quick clicks
        click1 = 0.0
        click2 = 0.0
        if t < 0.05:
            click1 = math.sin(2 * math.pi * 3000 * t) * math.exp(-t * 120)
        if 0.07 <= t < 0.12:
            t2 = t - 0.07
            click2 = math.sin(2 * math.pi * 2500 * t2) * math.exp(-t2 * 100)
        samples.append((click1 * 0.3 + click2 * 0.3))
    return samples


# ── Sound effect registry ───────────────────────────────────────────────

SOUNDS = [
    ("pistol_shot.wav", pistol_shot()),
    ("rifle_shot.wav", rifle_shot()),
    ("hit.wav", hit()),
    ("miss.wav", miss()),
    ("click.wav", click()),
    ("select.wav", select()),
    ("victory.wav", victory()),
    ("death.wav", death()),
    ("move.wav", move_sound()),
    ("reload.wav", reload()),
]


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    for filename, samples in SOUNDS:
        path = os.path.join(OUT_DIR, filename)
        write_wav(path, samples)
        size = os.path.getsize(path)
        print(f"  {filename}: {size} bytes ({len(samples)} samples)")

    print(f"\nGenerated {len(SOUNDS)} sound effects in {OUT_DIR}")


if __name__ == "__main__":
    main()
