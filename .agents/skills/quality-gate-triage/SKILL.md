---
name: quality-gate-triage
description: Diagnoses and fixes pasted finstack quality-gate failures, including mise pre-commit-run, Rust clippy/test failures, cargo deny, OSV, Python build/lint/type/parity failures, WASM build/test failures, and iterative bug-hunting loops. Use when the user shares terminal output, asks to fix CI/pre-commit/lint/test failures, or asks to find and fix bugs until checks are clean.
---

# Quality Gate Triage

Use this skill when the deliverable is a clean failing check or an iterative bug-fix loop. Prefer narrow evidence from the failing command over broad speculation.

## Routing

Use this skill for:

- pasted compiler, clippy, lint, typecheck, test, parity, `cargo deny`, OSV, maturin, WASM, or pre-commit output,
- requests like "fix this failure", "pre-commit is red", "CI failed", "run a bug hunt", or "keep going until the check is clean",
- bugs discovered while implementing another plan when the next step is blocked on a quality gate.

Use `quant-finance-review` first when the failure points to pricing/risk correctness. Use `binding-parity-reviewer` first when the failure is primarily cross-language API drift.

## Workflow

1. Capture the failing command and the first real error.
2. Identify the affected component: Rust, Python, WASM, bindings, docs, dependency audit, or parity.
3. Read the smallest relevant files and tests.
4. Fix the root cause, not downstream symptoms.
5. Re-run the smallest check that proves the fix.
6. If a new failure appears, repeat from the new first real error.
7. Stop only when the target check passes or a blocker requires user input.

## Finstack Command Map

Prefer repo tasks from `AGENTS.md` and `mise.toml`:

- Rust: `mise run rust-lint`, `mise run rust-test`
- Python: `mise run python-build`, `mise run python-lint`, `mise run python-test`, `mise run python-typecheck`
- WASM: `mise run wasm-build`, `mise run wasm-lint`, `mise run wasm-test`
- Broad gates: `mise run all-lint`, `mise run all-test`, `mise run all-ci`
- Pre-commit: `mise pre-commit-run` when the user is already running that gate

Use targeted checks while iterating. Reserve broad checks for broad changes or final verification.

## Bug-Hunting Mode

When the user asks for a bug hunt rather than a specific failing check:

1. Pick a focused scope instead of scanning the whole workspace blindly.
2. Run available static checks for that scope.
3. Trace suspicious code through callers and tests.
4. Fix real bugs in small slices.
5. Re-read touched files with fresh eyes.
6. Verify with targeted tests and report residual risk.

## Output Format

```markdown
## Triage Result

### Root cause
<first real error and why it happened>

### Fix
<files changed and behavior restored>

### Verification
- `<command>`: pass/fail

### Residual risk
<unchecked broad gates or unrelated blockers>
```

## Resources

- `references/TRIAGE.md` - triage decision tree.
- `references/PATTERNS.md` - common bug patterns.
- `references/TOOLS.md` - tool-specific notes.
- `outputs/quality-gate-report.md` - example completed triage report.
