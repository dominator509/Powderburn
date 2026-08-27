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
random.seed(0x1867)


def write_wav(path, samples, sample_rate=SAMPLE_RATE):
    """Write 16-bit mono WAV file from float samples in [-1, 1]."""
    num_samples = len(samples)
    data = bytearray()
    for s in samples:
        s = max(-32768, min(32767, int(s * 32767)))
        data.extend(struct.pack("<h", s))

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


def room(samples, echoes):
    """Add deterministic frontier-room reflections without clipping."""
    wet = list(samples)
    for delay_seconds, gain in echoes:
        delay = int(delay_seconds * SAMPLE_RATE)
        for i in range(delay, len(wet)):
            wet[i] += samples[i - delay] * gain
    peak = max(1.0, max(abs(sample) for sample in wet))
    return [sample * 0.92 / peak for sample in wet]


def soft_clip(value):
    """Analog-style saturation used by gun reports and music."""
    return math.tanh(value * 1.35) / math.tanh(1.35)


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


def damage_male(duration=0.28, sample_rate=SAMPLE_RATE):
    """Lower, breathy impact cry for the male atlas identity."""
    n = int(duration * sample_rate)
    rng = random.Random(0xD4A6)
    samples = []
    for i in range(n):
        t = i / sample_rate
        envelope = math.exp(-t * 10) * (1 - math.exp(-t * 180))
        fundamental = math.sin(2 * math.pi * (155 - 42 * t / duration) * t)
        rasp = math.sin(2 * math.pi * 310 * t) * 0.22
        noise = rng.uniform(-1, 1) * 0.11
        samples.append((fundamental * 0.60 + rasp + noise) * envelope)
    return samples


def damage_female(duration=0.28, sample_rate=SAMPLE_RATE):
    """Higher, breathier impact cry for the female atlas identity."""
    n = int(duration * sample_rate)
    rng = random.Random(0xFEA1E)
    samples = []
    for i in range(n):
        t = i / sample_rate
        envelope = math.exp(-t * 12) * (1 - math.exp(-t * 210))
        fundamental = math.sin(2 * math.pi * (390 - 120 * t / duration) * t)
        overtone = math.sin(2 * math.pi * 780 * t + 0.3) * 0.18
        breath = rng.uniform(-1, 1) * 0.17
        samples.append((fundamental * 0.48 + overtone + breath) * envelope)
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


def death_male(duration=0.75, sample_rate=SAMPLE_RATE):
    """Low descending, rough fall cue for the male atlas identity."""
    n = int(duration * sample_rate)
    samples = []
    for i in range(n):
        t = i / sample_rate
        freq = 220 - 140 * (t / duration)
        envelope = math.sin(math.pi * t / duration) ** 0.8
        tone = math.sin(2 * math.pi * freq * t) * 0.42
        rasp = math.sin(2 * math.pi * (freq * 2.01) * t) * 0.12
        samples.append((tone + rasp) * envelope)
    return samples


def death_female(duration=0.82, sample_rate=SAMPLE_RATE):
    """Higher descending, airy fall cue for the female atlas identity."""
    n = int(duration * sample_rate)
    rng = random.Random(0xDEAF)
    samples = []
    for i in range(n):
        t = i / sample_rate
        freq = 520 - 360 * (t / duration)
        envelope = math.sin(math.pi * t / duration) ** 0.75
        tone = math.sin(2 * math.pi * freq * t) * 0.34
        breath = rng.uniform(-1, 1) * 0.08 * math.exp(-t * 4)
        samples.append((tone + breath) * envelope)
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


def frontier_theme(duration=24.0, sample_rate=SAMPLE_RATE):
    """Original slow-burn frontier cue: guitar, reed melody, wind, and hoof pulse."""
    n = int(duration * sample_rate)
    chords = [
        (110.00, 164.81, 220.00),
        (87.31, 130.81, 174.61),
        (130.81, 196.00, 261.63),
        (98.00, 146.83, 196.00),
    ]
    melody = [440.00, 523.25, 493.88, 440.00, 392.00, 329.63, 392.00, 440.00]
    samples = []
    wind_state = 0.0
    for i in range(n):
        t = i / sample_rate
        bar = int(t / 3.0)
        chord = chords[bar % len(chords)]
        beat_t = t % 0.75
        pluck_env = math.exp(-beat_t * 4.8)
        guitar = 0.0
        for voice, freq in enumerate(chord):
            phase = 2.0 * math.pi * freq * t
            guitar += (
                math.sin(phase)
                + 0.42 * math.sin(phase * 2.0 + voice * 0.3)
                + 0.16 * math.sin(phase * 3.0)
            ) * pluck_env

        phrase_t = t % 6.0
        note_index = min(7, int(phrase_t / 0.75))
        note_t = phrase_t % 0.75
        reed_env = math.sin(math.pi * min(1.0, note_t / 0.62)) ** 2
        reed_freq = melody[(note_index + (bar // 2)) % len(melody)]
        vibrato = 1.0 + 0.006 * math.sin(2.0 * math.pi * 5.1 * t)
        reed = (
            math.sin(2.0 * math.pi * reed_freq * vibrato * t)
            + 0.24 * math.sin(4.0 * math.pi * reed_freq * t)
        ) * reed_env

        hoof_t = t % 1.5
        hoof = 0.0
        if hoof_t < 0.12 or 0.28 < hoof_t < 0.40:
            local_t = hoof_t if hoof_t < 0.12 else hoof_t - 0.28
            hoof = math.sin(2.0 * math.pi * 68.0 * local_t) * math.exp(-local_t * 28.0)

        wind_state = wind_state * 0.992 + random.uniform(-1.0, 1.0) * 0.008
        mix = guitar * 0.12 + reed * 0.10 + hoof * 0.07 + wind_state * 0.035
        fade = min(1.0, t / 1.5, (duration - t) / 1.5)
        samples.append(soft_clip(mix) * max(0.0, fade))
    return room(samples, [(0.19, 0.19), (0.37, 0.11), (0.61, 0.06)])


# ── Sound effect registry ───────────────────────────────────────────────

SOUNDS = [
    ("pistol_shot.wav", room(pistol_shot(0.65), [(0.08, 0.25), (0.19, 0.12)])),
    ("rifle_shot.wav", room(rifle_shot(0.85), [(0.11, 0.28), (0.27, 0.13)])),
    ("hit.wav", room(hit(0.28), [(0.07, 0.16)])),
    ("damage_male.wav", room(damage_male(), [(0.06, 0.12)])),
    ("damage_female.wav", room(damage_female(), [(0.06, 0.12)])),
    ("miss.wav", room(miss(0.34), [(0.05, 0.12)])),
    ("click.wav", click()),
    ("select.wav", room(select(0.18), [(0.06, 0.10)])),
    ("victory.wav", room(victory(1.4), [(0.16, 0.20), (0.31, 0.11)])),
    ("death.wav", room(death(0.75), [(0.12, 0.18)])),
    ("death_male.wav", room(death_male(), [(0.12, 0.18)])),
    ("death_female.wav", room(death_female(), [(0.14, 0.16)])),
    ("move.wav", room(move_sound(0.18), [(0.045, 0.08)])),
    ("reload.wav", room(reload(0.34), [(0.08, 0.12)])),
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
