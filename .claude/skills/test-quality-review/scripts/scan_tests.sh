#!/usr/bin/env bash
# scan_tests.sh — surface candidate test-quality smells in Rust + TS/Vitest tests.
#
# Usage:     bash scan_tests.sh [path ...]   (defaults: backend/src backend/tests frontend/src)
# Requires:  bash, grep, python3   (python3 powers the Rust no-assertion-body pass)
#
# Output is CANDIDATES, not verdicts. Every hit MUST be read in context before it
# becomes a finding — see ../references/anti-patterns.md § False-positive traps
# (custom assert helpers, joined spawns, complemented round-trips, MSW, etc.).
set -uo pipefail

PATHS=("$@")
if [ ${#PATHS[@]} -eq 0 ]; then
  PATHS=(backend/src backend/tests frontend/src)
fi

RUST_PATHS=(); TS_PATHS=()
for p in "${PATHS[@]}"; do
  [ -e "$p" ] || continue
  case "$p" in
    *frontend*) TS_PATHS+=("$p") ;;
    *backend*)  RUST_PATHS+=("$p") ;;
    *)          RUST_PATHS+=("$p"); TS_PATHS+=("$p") ;;
  esac
done

section() { printf '\n=== %s ===\n' "$1"; }

if [ ${#RUST_PATHS[@]} -gt 0 ]; then
  section "RUST [1] variant-only Result asserts (is_ok/is_err/is_some/is_none) — weak unless single-failure-mode"
  grep -rnE "assert!\(.*\.(is_ok|is_err|is_some|is_none)\(\)" "${RUST_PATHS[@]}" 2>/dev/null
  section "RUST [2] #[should_panic] WITHOUT expected= (passes on ANY panic)"
  grep -rn "should_panic" "${RUST_PATHS[@]}" 2>/dev/null | grep -v "expected"
  section "RUST [3] assert!(true) / assert!(false)"
  grep -rnE "assert!\((true|false)\)" "${RUST_PATHS[@]}" 2>/dev/null
  section "RUST [4] #[ignore] (never runs in CI)"
  grep -rn "#\[ignore" "${RUST_PATHS[@]}" 2>/dev/null
  section "RUST [5] tokio::spawn — confirm the JoinHandle is awaited/joined"
  grep -rnE "tokio::spawn|task::spawn" "${RUST_PATHS[@]}" 2>/dev/null
  section "RUST [6] Debug/Display string asserts (implementation detail, not behavior)"
  grep -rnE "assert_eq!\(format!\(\"\{:\?\}|to_string\(\), ?\"" "${RUST_PATHS[@]}" 2>/dev/null
  section "RUST [7] #[test]/#[tokio::test] fns with NO assertion tokens — TRACE HELPERS before trusting"
  python3 - "${RUST_PATHS[@]}" <<'PY'
import re, os, sys
roots = sys.argv[1:]
files = []
for base in roots:
    if os.path.isfile(base) and base.endswith('.rs'):
        files.append(base)
    elif os.path.isdir(base):
        for r, _, fs in os.walk(base):
            for f in fs:
                if f.endswith('.rs'):
                    files.append(os.path.join(r, f))
# Tokens that count as "a real check" — includes common custom-assert helpers so we
# don't false-flag (axum-test .assert_status/.assert_json, insta snapshots, etc.).
tok = re.compile(r'assert!|assert_eq!|assert_ne!|prop_assert|\.unwrap\(\)|\.expect\(|'
                 r'panic!|unreachable!|\bErr\(|should_panic|insta|assert_snapshot|'
                 r'expect!|\.assert_|assert_status|assert_json|assert_text')
attr = re.compile(r'#\[(tokio::test|test|sqlx::test|rstest)\]')
for path in sorted(set(files)):
    try:
        src = open(path, encoding='utf-8', errors='replace').read().splitlines()
    except OSError:
        continue
    i = 0
    while i < len(src):
        if attr.search(src[i]):
            j = i
            while j < len(src) and 'fn ' not in src[j]:
                j += 1
            if j >= len(src):
                break
            depth = 0; started = False; body = []; k = j
            while k < len(src):
                depth += src[k].count('{') - src[k].count('}')
                body.append(src[k])
                if '{' in src[k]:
                    started = True
                if started and depth <= 0:
                    break
                k += 1
            if not tok.search("\n".join(body)):
                m = re.search(r'fn\s+(\w+)', src[j])
                print(f"  {path}:{j+1}  fn {m.group(1) if m else '?'}  (no assertion tokens — verify it is truly empty / not a helper)")
            i = k + 1
        else:
            i += 1
PY
fi

if [ ${#TS_PATHS[@]} -gt 0 ]; then
  section "TS [1] bare vi.mock (automock — exports become undefined)"
  grep -rnE "vi\.mock\(['\"][^'\"]+['\"]\)[[:space:]]*;?[[:space:]]*$" "${TS_PATHS[@]}" 2>/dev/null
  section "TS [2] vi.spyOn — confirm it isn't the SUT and isn't the SOLE assertion"
  grep -rn "vi.spyOn(" "${TS_PATHS[@]}" 2>/dev/null
  section "TS [3] weak matchers (toBeTruthy/toBeFalsy/toBeDefined/anything)"
  grep -rnE "\.(toBeTruthy|toBeFalsy|toBeDefined)\(\)|expect\.anything\(\)" "${TS_PATHS[@]}" 2>/dev/null
  section "TS [4] .not.toThrow() used as the assertion"
  grep -rn "not.toThrow(" "${TS_PATHS[@]}" 2>/dev/null
  section "TS [5] empty waitFor, or waitFor/findBy not awaited"
  grep -rnE "waitFor\(\(\)[[:space:]]*=>[[:space:]]*\{\}\)" "${TS_PATHS[@]}" 2>/dev/null
  grep -rnE "(^|[^a-zA-Z.])(waitFor|findBy[A-Za-z]+)\(" "${TS_PATHS[@]}" 2>/dev/null | grep -vE "await (waitFor|screen\.findBy|findBy)" | grep -v "import"
  section "TS [6] focused/disabled tests (.only/.skip/.todo/xit/fit)"
  grep -rnE "\b(it|test|describe)\.(only|skip|todo)\b|\bf(it|describe)\(|\bx(it|describe|test)\(" "${TS_PATHS[@]}" 2>/dev/null
  section "TS [7] snapshots (rubber-stamp risk)"
  grep -rnE "toMatchSnapshot\(|toMatchInlineSnapshot\(|toThrowErrorMatchingSnapshot\(" "${TS_PATHS[@]}" 2>/dev/null
  section "TS [8] toHaveBeenCalled* count per file (sole assertion ⇒ testing the mock)"
  grep -rcE "toHaveBeenCalled" "${TS_PATHS[@]}" 2>/dev/null | grep -vE ":0$"
fi

printf '\nDONE — every hit is a CANDIDATE. Read it in context before flagging (references/anti-patterns.md § False-positive traps).\n'
