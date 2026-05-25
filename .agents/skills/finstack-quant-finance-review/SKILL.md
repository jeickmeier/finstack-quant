---
name: finstack-quant-finance-review
description: >
  Use when reviewing quant finance code, pricing models, trading logic, risk
  calculations, calibration, market convention handling, or numerical
  stability in Rust, Python, WASM, or SQL. Trigger for full-workspace quant
  production audits, line-level correctness reviews, pricing/risk defect
  hunts, and benchmark-convention checks.
---

# Quant Finance Review

Review code like a senior quantitative developer responsible for getting prices, risk, and conventions right in production. Default to a practitioner mindset: would this hold up in a live book, against market standards, and under stress at end-of-day risk time?

## Audit Discipline

- Inspect the available code before giving findings. If repository files are present, do not answer with only a generic audit framework.
- For `rfin`, start broad with `cargo metadata --no-deps --format-version 1`, then trace the affected crates, bindings, parity contract, tests, and examples.
- Ground every finding in a concrete file and line when code is available. If code is unavailable, state that limitation and provide only a search plan/checklist.
- Separate current-production benchmark standards from legacy workflows. Treat LIBOR as legacy-only unless the code explicitly models fallback or historical contracts.
- Prefer targeted verification tied to the defect over slow whole-workspace runs unless the change crosses crate or binding boundaries.

## When to Use

- Pricing, risk, calibration, cashflow, or market data code needs review
- The user asks about numerical stability, model correctness, or production readiness
- The review needs market convention validation, not just clean code feedback
- The code is in Rust, Python, WASM/JS, or SQL and touches quantitative finance behavior
- The user wants a library assessment, gap analysis, or benchmark comparison against professional tools

## Review Priorities

Weight these in order unless the user narrows the scope:

1. **Mathematical correctness**
   Verify formulas, sign conventions, payoff logic, discounting, accrual, and edge conditions.
2. **Market convention compliance**
   Treat day counts, calendars, settlement lags, roll rules, premium currency, and curve usage as first-class review targets.
3. **Numerical robustness**
   Check finite-difference bumps, solver convergence, Monte Carlo error control, interpolation boundaries, and floating-point stability.
4. **Production readiness**
   Look for deterministic behavior, input validation, missing observability, bad failure modes, and missing auditability.
5. **Performance and ergonomics**
   Review hot loops, allocations, vectorization, parallelism, and whether the API matches how a desk would actually use it.

## Language Lens

- **Rust**: ownership and lifetime safety, allocation patterns, trait boundaries for numerical abstractions, SIMD and cache behavior
- **Python**: vectorization, NumPy/SciPy idioms, `math.fsum` vs naive accumulation, dtype drift, and reproducibility
- **WASM/JS**: precision loss from JS numbers, serialization overhead, and cross-runtime numerical consistency
- **SQL**: join semantics for market data, null handling in financial calculations, and time-series query correctness

## Review Modes

### Targeted review
Use when the user points at a file, pricer, risk routine, or asset class. Go deep on correctness, conventions, and edge cases for that scope.

### Library assessment
Use when the user asks for a broader audit or gap analysis. Check coverage, extensibility, defaults, and whether the library is credible against QuantLib, Bloomberg, or standard desk expectations.

### Numerical regression review
Use when the user asks about golden tests, tolerance drift, solver stability, finite-difference Greeks, calibration reproducibility, NaN/inf handling, or benchmark/reference price comparisons. Open `references/numerical-regression.md` and report the exact regression surface and tolerance policy.

### Market convention review
Use when the user asks about day counts, calendars, settlement lags, fixing publication, curve roles, shock units, clean/dirty price, FX policy, or credit curve conventions. Open the relevant `market-standards/` file and `references/finstack-module-index.md`.

## Workflow

1. **Scope the instrument and asset class**
   Identify whether the code is rates, FX, credit, fixed income, equity, commodities, or trading-system logic.
2. **Check math before style**
   Wrong formula, wrong sign, wrong convention, or unstable numerics outrank maintainability concerns.
3. **Audit conventions explicitly**
   Pull the relevant market standards file and verify day counts, settlement, calendars, roll rules, accrual, premium conventions, and curve assignment.
4. **Check numerical evidence**
   Look for edge-case tests, parity checks, round-trips, benchmark comparisons, and convergence diagnostics.
5. **Assess production behavior**
   Ask whether the code fails loudly, explains bad outputs, and can be trusted in a portfolio run.
6. **Report findings first**
   Lead with bugs, regressions, and material risks ordered by severity. Keep summary text brief.

## Full-Workspace Audit Workflow

1. **Map the workspace**
   Run `cargo metadata --no-deps --format-version 1` and identify canonical Rust crates, Python bindings, WASM bindings, parity contracts, tests, benches, and examples.
2. **Prioritize financial invariants**
   Search first for valuation date, discounting, day-count, calendars, curve selection, fixings, clean/dirty price, shock units, solver failures, and NaN handling.
3. **Trace canonical paths**
   Follow Rust implementation -> Rust tests -> PyO3/WASM wrappers -> stubs/types -> parity contract -> examples/notebooks for exposed behavior.
4. **Use the production audit playbook**
   Open `references/production-audit-playbook.md` for search patterns and defect classes when doing broad audits or when initial findings are sparse.
5. **Verify with the narrowest meaningful commands**
   Run focused Rust tests, parity checks, binding topology audits, or format/lint commands that prove the reviewed surface. State any unrelated blockers exactly.

## Severity Guide

- **Blocker**: incorrect price, P&L, risk, or trade economics; broken market standard; materially wrong benchmark result
- **Major**: numerical instability, bad edge-case behavior, missing market-standard feature, or weak production safeguards
- **Moderate**: performance or API issue that would hurt adoption or scale but not immediately break valuations
- **Minor**: naming, docs, polish, or non-critical implementation cleanup

## Output Format

Structure reviews like this:

```
## Findings
Ordered by severity. Each finding should include location, issue, impact, and the concrete fix.

## Open Questions or Assumptions
Anything that blocks certainty about pricing, conventions, calibration, or production behavior.

## Brief Summary
Only after findings. Capture overall quality, major strengths, and residual risk.

## Quant Notes
Reference literature, benchmark expectations, and domain-specific follow-ups when helpful.
```

## Reference Map

Use the lighter files first; open the deeper references only when needed.

### Market standards
- `market-standards/fx-standards.md`
- `market-standards/rates-standards.md`
- `market-standards/fixed-income-standards.md`
- `market-standards/equity-standards.md`
- `market-standards/algorithm-standards.md`
- `market-standards/cross-asset-checklist.md`

### Quant references
- `references/production-audit-playbook.md`
- `references/finstack-module-index.md`
- `references/numerical-regression.md`
- `references/numerical-methods.md`
- `references/pricing-models.md`
- `references/risk-models.md`
- `references/trading-systems.md`
- `references/language-patterns.md`
- `references/reference.md`
- `references/examples.md`

## Practitioner Heuristics

- Conventions kill quietly. Perfect math with the wrong day count still produces bad P&L.
- Test boring paths aggressively: schedules, accruals, payment dates, stubs, and fixings.
- Benchmarks matter. Prefer cross-checks against QuantLib, Bloomberg, ISDA, or authoritative formulas.
- A review is incomplete if it ignores edge cases like expiry, zero vol, negative rates, or deep ITM/OTM regimes.
- Prefer concrete findings over generic commentary. "Uses projection curve for discounting" beats "curve logic may be wrong."
