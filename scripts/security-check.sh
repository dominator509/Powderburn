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
if git grep -InE '(api[_-]?key|secret|password|token)[[:space:]]*=[[:space:]]*\"[A-Za-z0-9_/+-]{16,}\"' -- crates content scripts >/dev/null 2>&1; then
  fail "hardcoded credential-shaped literal in tracked source"
fi

# 2. LBI-09 NO NETWORK. The shipped binary must contain no socket syscall surface.
_check_binary_for_network_syms() {
  for binary in target/release/powderburn target/debug/powderburn target/release/pbtool; do
    [ -x "$binary" ] || continue
    syms=$(nm -uC "$binary" 2>/dev/null || true)
    for s in socket connect getaddrinfo gethostbyname SSL_connect curl_easy_init \
             sendto recvfrom bind listen accept; do
      case "$s" in
        socket)
          # Exact match for 'socket' appearing as a word (exclude socketpair)
          match=$(echo "$syms" | tr ' ' '\n' | grep -x '.*\bsocket\b.*' 2>/dev/null || true)
          [ -n "$match" ] || continue
          # Check reality-allow list
          if [ -f .agent/reality-allow ] && grep -q "${binary}:${s}:" .agent/reality-allow 2>/dev/null; then
            continue
          fi
          fail "binary $binary references network symbol: $s (LBI-09)"
          ;;
        connect)
          match=$(echo "$syms" | tr ' ' '\n' | grep -x '.*\bconnect\b.*' 2>/dev/null || true)
          [ -n "$match" ] || continue
          # Check reality-allow list
          if [ -f .agent/reality-allow ] && grep -q "${binary}:${s}:" .agent/reality-allow 2>/dev/null; then
            continue
          fi
          fail "binary $binary references network symbol: $s (LBI-09)"
          ;;
        *)
          case "$syms" in *"$s"*) fail "binary $binary references network symbol: $s (LBI-09)";; esac
          ;;
      esac
    done
  done
}
_check_binary_for_network_syms

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
# Check that new limit constants exist
for f in crates/pb-cli/src/journal.rs crates/pb-app/src/settings.rs; do
  [ -f "$f" ] || fail "missing file: $f"
  grep -q 'MAX_' "$f" || fail "$f has no explicit size limit constant (SECURITY.md section 5)"
done
if grep -RInE 'unsafe[[:space:]]*\{' crates --include='*.rs' | grep -v '// SAFETY:' >/dev/null 2>&1; then
  fail "unsafe block without a SAFETY comment"
fi

# 4. Log redaction. No filesystem path or environment value reaches a log line verbatim.
if grep -RInE 'log::(info|warn|error)!\([^)]*env!' crates --include='*.rs' >/dev/null 2>&1; then
  fail "environment value interpolated into a log line"
fi

# 5. Redaction module exists and has tests.
if [ -f crates/pb-core/src/redact.rs ]; then
  grep -q 'fn redact_path' crates/pb-core/src/redact.rs || fail "redact.rs missing redact_path function"
  grep -q 'fn redact_user' crates/pb-core/src/redact.rs || fail "redact.rs missing redact_user function"
  grep -q '#\[cfg(test)\]' crates/pb-core/src/redact.rs || fail "redact.rs missing test module"
else
  fail "crates/pb-core/src/redact.rs does not exist (SECURITY.md section 8)"
fi

# 6. Dependency posture.
sh scripts/dependency-audit.sh >/dev/null

echo "security-check: ok"
