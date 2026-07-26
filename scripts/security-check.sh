#!/usr/bin/env sh
# Hardening gate. Enforces the SECURITY.md checklist mechanically.
set -eu
: "${PB_HOME:?PB_HOME must be set}"
cd "$PB_HOME"
fail() { echo "security-check: FAIL - $1" >&2; exit 1; }

# 1. No secret material committed, ever.
if git ls-files | grep -qx '.env'; then fail "the .env file is tracked by git"; fi
if git grep -InE 'BEGIN (RSA|OPENSSH|PGP|EC) PRIVATE KEY' -- . >/dev/null 2>&1; then
  fail "private key material found in tracked files"
fi
if git grep -InE '(api[_-]?key|secret|password|token)[[:space:]]*=[[:space:]]*"[A-Za-z0-9_/+-]{16,}"' -- crates content scripts >/dev/null 2>&1; then
  fail "hardcoded credential-shaped literal in tracked source"
fi

# 2. LBI-09 NO NETWORK. The shipped binary must contain no socket syscall surface.
if [ -x target/release/powderburn ]; then
  syms=$(nm -uC target/release/powderburn 2>/dev/null || true)
  for s in socket connect getaddrinfo gethostbyname SSL_connect curl_easy_init; do
    case "$syms" in *"$s"*) fail "release binary references network symbol: $s (LBI-09)";; esac
  done
fi
if grep -RInE 'std::net|TcpStream|UdpSocket|reqwest|hyper::' crates --include='*.rs' \
   | grep -v 'crates/pb-cli/src/replay_server.rs' >/dev/null 2>&1; then
  fail "network API used outside the feature-gated replay server (LBI-09)"
fi
if grep -RIn 'replay-server' crates/pb-app/Cargo.toml >/dev/null 2>&1; then
  fail "the replay-server feature is reachable from the shipped app crate"
fi

# 3. Untrusted input handling. Saves, journals, and data mods are parsed with limits.
for f in crates/pb-save/src/load.rs crates/pb-content/src/load.rs; do
  [ -f "$f" ] || fail "missing untrusted-input parser: $f"
  grep -q 'MAX_' "$f" || fail "$f has no explicit size limit constant (SECURITY.md section 5)"
done
if grep -RInE 'unsafe[[:space:]]*\{' crates --include='*.rs' | grep -v '// SAFETY:' >/dev/null 2>&1; then
  fail "unsafe block without a SAFETY comment"
fi

# 4. Log redaction. No filesystem path or environment value reaches a log line verbatim.
if grep -RInE 'log::(info|warn|error)!\([^)]*env!' crates --include='*.rs' >/dev/null 2>&1; then
  fail "environment value interpolated into a log line"
fi

# 5. Dependency posture.
sh scripts/dependency-audit.sh >/dev/null

echo "security-check: ok"
