# Adapter recipe: grafting any future agent platform onto POWDERBURN

The pack is platform-agnostic. All real content lives in AGENTS.md and under `.agent/`. An adapter is
nothing but a pointer, so adding a platform takes exactly two steps.

## Step 1. Find where the platform reads standing instructions

Consult that platform's current documentation for the file it loads automatically at session start.
Known paths already shipped in this repository:

| Platform | Path |
| --- | --- |
| Claude Code | CLAUDE.md |
| Codex CLI and the generic convention | AGENTS.md itself |
| Gemini CLI | GEMINI.md |
| GitHub Copilot | .github/copilot-instructions.md |
| Cursor | .cursor/rules/6layer.mdc |
| Cline | .clinerules/6layer.md |
| Hermes | .hermes/instructions.md |
| OpenClaw | .openclaw/instructions.md |

## Step 2. Place the PRIME BLOCK there verbatim

Copy the region between `PRIME-BLOCK-BEGIN` and `PRIME-BLOCK-END` from AGENTS.md, byte for byte,
into the new file. Add at most one line above it naming the platform. Add nothing else. Adapters
carry zero volatile state so they stay byte-stable for an entire run, which is what keeps prefix
caches warm.

## Step 3. Extend the parity gate

Append the new path to the adapter parity check command in COMMANDS.md and to the same command in
`.agent/checklists/final-review.md`. Run it. All cksum lines must match.

## Why nothing else is ever needed

The adapter does not carry rules, commands, or state. It carries the boot sequence. The boot sequence
sends the agent to AGENTS.md, COMMANDS.md, .agent/GRAPH.md, .agent/LOOPS.md, the ledger, and
`scripts/graph-next.sh`. Any agent that can read files, edit files, and run commands is then fully
equipped, alone or alongside others, because the repository is the only channel.
