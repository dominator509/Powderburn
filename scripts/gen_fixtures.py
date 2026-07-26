#!/usr/bin/env python3
"""Generate adversarial test fixtures."""
import os

fixtures_dir = os.path.join(os.path.dirname(__file__), '..', 'tests', 'fixtures', 'adversarial')
os.makedirs(fixtures_dir, exist_ok=True)

# ---- bomb.ron: 8000 records ----
lines = ["["]
for i in range(8000):
    lines.append(f'  (id: "r_{i:04}", value: {i}, data: "benchmark_record_{i}"),')
lines.append("]")
with open(os.path.join(fixtures_dir, "bomb.ron"), "w") as f:
    f.write("\n".join(lines))

# ---- deep_nest.ron: 65 levels of nesting ----
# Use nested tuples: ((((...(42)...))))
depth = 65
inner = "42"
for _ in range(depth):
    inner = f"({inner},)"
with open(os.path.join(fixtures_dir, "deep_nest.ron"), "w") as f:
    # Wrap in a list for RON file format
    f.write(f"[{inner}]\n")

print("Created bomb.ron and deep_nest.ron")
