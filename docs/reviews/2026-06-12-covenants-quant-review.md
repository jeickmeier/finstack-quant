# Quant Finance Review — `finstack-quant/covenants` Crate and Bindings

> **Remediation status (2026-06-12): NOT STARTED.** Review complete; all
> Blocker/Major findings below were empirically confirmed with throwaway
> probe tests (run against the crate, output captured, then deleted — the
> working tree contains no review artifacts). Crate test suite green at
> review time: 52 tests across 4 suites.

**Date:** 2026-06-12
**Scope:** `finstack-quant/covenants` (engine, forward forecasting, schedule, metric,
report, templates, json), `finstack-quant-py/src/bindings/covenants`,
`finstack-quant-wasm/src/api/covenants`, JS facade (`exports/covenants.js`),
Python stub (`finstack-quant/covenants/__init__.pyi`), parity contract entries,
and all crate/binding tests (~2,800 source lines reviewed in full).

---

## Findings

### Blocker

#### B1. Breach forecasting ignores the negative-EBITDA (NM ratio) convention the engine enforces

Distressed borrowers show huge headroom and 0% breach probability in forecasts.

The point-in-time engine correctly treats a negative leverage-type ratio as a
breach (`engine.rs:1093-1102`, via `CovenantType::is_ratio_max`, with a
dedicated regression test in `tests/engine_conventions.rs`). The forward path
never got that fix — three sites use plain `v > t`:

- deterministic breach test: `forward.rs:221-232`
- MC non-positive-base fallback: `forward.rs:302-311`
- `first_breach_date`: `forward.rs:363-375`

**Probe confirmed:** `MaxDebtToEBITDA <= 4.0` with metric `-10.0` (negative
EBITDA) forecasts headroom **+3.5 (+350%)**, breach probability **0.0**, no
first breach — in both deterministic and stochastic modes — while
`CovenantEngine::evaluate` flags the identical input as breached.

This is a materially wrong risk signal in exactly the distressed scenario
where covenant forecasting matters most.

**Fix:** apply `is_ratio_max() && v < 0.0 ⇒ breached` (headroom NaN or −∞
convention) in all three forward-path sites, mirroring the engine.

### Major

#### M1. `forecast_breaches_generic` hard-fails the entire batch on any non-numeric covenant

The crate's own `cov_lite` template is unforecastable.

`metric_value_for_spec` returns `Some(1.0)` for `Negative`/`Affirmative`
(`forward.rs:496`), so they always count as "covered"; then
`forecast_covenant_generic` errors on missing `bound_kind`
(`forward.rs:162-170`) and the `?` at `forward.rs:441` propagates, failing
the whole engine batch.

**Probe confirmed:** `templates::cov_lite(6.0, 4.0)` →
`Err(Validation("covenant 'Negative: …' has no bound kind"))`.

**Fix:** skip non-numeric covenants in the batch loop (with a
`tracing::warn!`), consistent with how missing metrics are already skipped.

#### M2. Persistent breach re-recorded with fresh cure deadline at every test date; consequences re-applied per record

`evaluate_and_track`'s dedup requires `breach_date == test_date`
(`engine.rs:864-868`), so one continuous breach tested quarterly produces one
record per quarter, each with a new `test_date + cure_period` deadline.
`apply_consequences`' double-application guard also matches on `breach_date`
(`engine.rs:929-942`), so each record fires independently.

**Probe confirmed:** one continuous Debt/EBITDA breach evaluated at three
quarter-ends → 3 breach records → 3 separate applications of the 200bp
`RateIncrease` (600bp total) for one covenant breach.

The rolling fresh deadlines also mean "In cure period" can display
indefinitely for a never-cured breach.

**Fix:** treat an existing uncured breach for the same instance key as the
*same* breach (don't re-record while one is active), anchoring the cure clock
to the original breach date. LSTA-style credit agreements run cure periods
from breach/notice, not from each subsequent compliance certificate.

#### M3. Documented cure-by-recovery is not implemented — `is_cured` is never set

The module contract says a breach can be neutralized "by the metric
recovering before the cure deadline" (`engine.rs:15-18`), but no code path
sets `is_cured = true`: if a Q1 breach recovers at Q2 (inside the cure
window), the Q1 record stays uncured and `apply_consequences` past the
deadline still applies Default/RateIncrease. Callers must mutate
`breach_history` by hand.

**Fix:** in `evaluate_and_track`, mark active breaches cured when the
covenant passes on a test date ≤ its cure deadline (or expose an explicit
`cure_breach` API and fix the doc).

#### M4. Inbound JSON does not deny unknown fields — "validate" entry points silently drop misspelled keys, including waivers

Workspace invariant: "unknown fields denied on inbound types"
(`deny_unknown_fields` is the norm in sibling crates, e.g.
statements-analytics ECL types, factor-model config). None of the covenant
types have it.

**Probe confirmed:** `validate_covenant_engine_json` with a `"waviers"` typo
returns `Ok` and canonicalizes to an engine with **empty waivers** — a waived
covenant would then report as breached, or a typo'd amended threshold would
silently revert to the stricter base. For functions whose sole purpose is
validation (`json.rs:21-33`), this defeats the contract.

**Fix:** add `#[serde(deny_unknown_fields)]` to `CovenantEngine`,
`CovenantSpec`, `Covenant`, `CovenantWaiver`, `CovenantWindow`,
`CovenantBreach`, `SpringingCondition`, `CovenantForecastConfig`,
`CovenantReport`.

### Moderate

#### Mo1. Non-finite values break the JSON wire format

Inactive springing periods set headroom to `+∞` (`forward.rs:234-239`);
serde_json emits `null` inside `Vec<f64>` and `min_headroom_value`.

**Probe confirmed:** the serialized `CovenantForecast` fails to deserialize
(`"headroom":[null]`). NaN projected values (the documented NaN-⇒-breach
path) hit the same problem.

**Fix:** finite sentinel or `Option<f64>`/custom serializer for these fields.

#### Mo2. Min-headroom scan is not NaN-robust

`forward.rs:354-361`: if `headroom[0]` is NaN every IEEE comparison is false,
so `min_idx` sticks at 0 and the true minimum-headroom date across later
finite periods is lost.

#### Mo3. `CovenantForecast.covenant_id` holds the display description, not the stable identity key

`forward.rs:155` uses `description()` (e.g. `"Debt/EBITDA <= 3.00x"` —
asserted by the test at `forward.rs:793`). Forecasts can't be joined to
engine reports/breach history keyed on `instance_key()`, and the threshold
baked into the string is wrong whenever a schedule or waiver overrides it.

**Fix:** emit `instance_key()`; keep the description in a separate field.

#### Mo4. The Monte Carlo machinery reproduces a closed-form quantity

Under the lognormal shock `base·exp(−σ²T/2 + σ√T·Z)`, the per-date breach
probability is exactly `Φ(±(ln(thr/base) + σ²T/2)/(σ√T))` — PhiloxRng,
antithetic variates, and stderr estimation add cost and sampling noise for an
analytic number. The analytic formula could replace ~80 lines, or at least be
the default with MC retained only if path-consistent simulation is planned.

Relatedly, draws across test dates are independent marginals (not a path), so
only per-date probabilities are meaningful — no first-passage distribution
exists, and `first_breach_date` is deterministic-only; deserves a doc note.

#### Mo5. Identity doc drift on waivers/breaches

`CovenantWaiver.covenant_id` and `CovenantBreach.covenant_id` docs say the
key comes "from `CovenantType::covenant_id`" (`engine.rs:451`,
`engine.rs:578-580`), but matching is against `instance_key()` (label-first,
`engine.rs:1055-1057`, `engine.rs:1143-1149`). A waiver written with the type
id will silently fail to match a labeled covenant — the covenant tests as
breached despite a granted waiver.

#### Mo6. Window fallback and overlap semantics

When windows exist but the test date falls outside all of them, the engine
silently evaluates *all* base specs (`engine.rs:982-992`) — arguably it
should evaluate none or make the fallback explicit. Window overlap is only a
`debug_assert!` (`engine.rs:664-675`); release builds accept overlapping
windows with first-match-wins.

### Minor

- **Mi1.** Antithetic stderr `sqrt(p(1−p)/n)` with `n = effective_paths`
  (`forward.rs:347-349`) ignores pair correlation; compute on pair-averaged
  indicators (n/2 units) or document conservativeness.
- **Mi2.** Degenerate stochastic config is silent: `stochastic=true` with
  `volatility=None` → σ=0 (deterministic result dressed as MC);
  `num_paths=0` → clamped to 1 path with stderr 0. Validate and error
  (`forward.rs:250-252`).
- **Mi3.** `forecast_breaches_generic` reports any period with `prob > 0.0`
  as a `FutureBreach` (`forward.rs:450`) — at realistic vols that's every
  period; a probability threshold belongs in the config.
- **Mi4.** MC time scaling hardcodes ACT/365.25 (`forward.rs:313`) instead
  of core day-count conventions; acceptable for shock scaling but worth a
  doc note.
- **Mi5.** `cure_period_days: Option<i32>` accepts negative values without
  validation (deadline before breach date).
- **Mi6.** `MaxCapex`/`MinLiquidity`/`Basket` thresholds are bare `f64`
  amounts with no currency tag — given the workspace's currency-safety
  invariant, at minimum document the deal-currency assumption.
- **Mi7.** Python `__all__` in registration isn't sorted
  (`finstack-quant-py/src/bindings/covenants/mod.rs:135-147`); binding standards
  require sorted, and the `.pyi` stub *is* sorted.
- **Mi8.** No WASM facade tests exercise the covenants namespace (nothing in
  `finstack-quant-wasm/tests/` touches it; WASM standards checklist requires
  facade tests). Python parity has 2 covenant tests; thin but present.
- **Mi9.** `ThresholdSchedule` accepts duplicate dates; last entry wins
  silently on lookup — document or reject.

---

## Open Questions or Assumptions

- Is `forecast_breaches_generic`/`forecast_covenant_generic` reachable from a
  production reporting path today, or only via the statements bridge in the
  meta crate? B1 stands either way, but it determines the urgency of M1
  versus just fixing the convention.
- M2/M3 assume the intended semantics are "one continuous breach = one breach
  record, curable by recovery" — the standard credit-agreement reading. If
  the design intent is per-test-date breach events, the
  consequence-application guard needs to dedupe at the instance-key level
  instead, and the doc contract needs rewriting.
- The bindings expose only the JSON slice (no forecasting), consistent with
  the parity contract's "first binding slice" note — assumed deliberate.

## Brief Summary

The crate is well-shaped for its size: the point-in-time engine has clearly
thought-through conventions (NM-ratio breaches, NaN-⇒-breach, IEEE infinity
semantics, instance-key collision rejection, headroom sign for negative
thresholds), each backed by a targeted regression test, and the bindings are
exemplary thin wrappers (GIL released, parity contract registered,
stub/facade in sync). The problem is that the **forward-forecasting module
and the breach-lifecycle tracking did not keep pace with the engine's
conventions**: the forecast contradicts the engine in the distressed regime
(B1), batch forecasting breaks on stock templates (M1), and a continuous
breach compounds consequences quarterly (M2). Residual risk concentrates
entirely in `forward.rs` and the `evaluate_and_track`/`apply_consequences`
lifecycle; the evaluation core itself is solid.

## Quant Notes

- The negative-EBITDA "NM" convention matches rating-agency practice
  (S&P/Moody's report leverage as "NM" when EBITDA ≤ 0) — the engine's
  treatment is right; the forecast just needs to inherit it.
- Analytic breach probability: with `X_T = base·exp(−σ²T/2 + σ√T Z)`,
  `P(X_T > thr) = Φ((ln(base/thr) − σ²T/2)/(σ√T))` — exact, deterministic,
  and removes seed/path-count from the config surface. If first-breach
  *timing* distributions are a roadmap item, that requires genuine
  path-consistent simulation (the current per-date independent draws cannot
  produce them).
- Cure-period mechanics in leveraged loan credit agreements (LSTA-style) run
  from the earlier of borrower knowledge/notice of the breach, not from each
  subsequent compliance certificate date — supporting the "anchor to first
  breach" fix in M2. Equity-cure rights (typically limited to 2-of-4
  quarters) are explicitly out of scope per the module docs, which is a fair
  v1 boundary.
