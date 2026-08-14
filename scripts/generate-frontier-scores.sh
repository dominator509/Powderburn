#!/usr/bin/env bash
set -euo pipefail

output_dir="${1:?usage: generate-frontier-scores.sh OUTPUT_DIR}"
mkdir -p "$output_dir"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

make_score() {
    local output="$1"
    local tempo_seconds="$2"
    shift 2
    local index=0
    local segments=()
    while (($# >= 2)); do
        local root="$1"
        local fifth="$2"
        shift 2
        local segment="$work_dir/segment-${index}.wav"
        sox -n -r 44100 -b 16 -c 2 "$segment" \
            synth "$tempo_seconds" pluck "$root" pluck "$fifth" \
            fade 0.03 "$tempo_seconds" 0.8 gain -9
        segments+=("$segment")
        index=$((index + 1))
    done
    sox "${segments[@]}" "$work_dir/strings.wav"
    local duration
    duration="$(soxi -D "$work_dir/strings.wav")"
    sox -n -r 44100 -b 16 -c 2 "$work_dir/air.wav" \
        synth "$duration" brownnoise brownnoise \
        highpass 90 lowpass 900 tremolo 0.22 18 gain -36
    sox -m "$work_dir/strings.wav" "$work_dir/air.wav" \
        -r 44100 -b 16 -c 2 "$output" reverb 22 35 35 18 5 1
}

make_score "$output_dir/dust_and_ashes.wav" 6 \
    A2 E3 C3 G3 F2 C3 E2 B2 \
    A2 E3 C3 G3 D3 A3 E2 B2

make_score "$output_dir/winter_count.wav" 7 \
    D3 A3 B2 F3 G2 D3 A2 E3 \
    D3 A3 C3 G3 B2 F3 A2 E3

make_score "$output_dir/iron_road.wav" 5 \
    E2 B2 G2 D3 A2 E3 C3 G3 \
    E2 B2 D3 A3 C3 G3 B2 F3

make_score "$output_dir/hard_road.wav" 3 \
    E2 B2 E2 C3 G2 D3 A2 E3 \
    E2 B2 C3 G3 D3 A3 B2 F3

printf 'Generated four original offline frontier scores in %s\n' "$output_dir"
