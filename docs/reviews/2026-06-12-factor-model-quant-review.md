# Quant Finance Review — `finstack-quant/factor-model` Crate and Bindings

> **Remediation status (2026-06-12): COMPLETE for all Majors and the actionable
> Moderates/Minors.** Highlights: M1 asof-coverage hard error (both directions) +
> strictly-increasing date validation; M2 `'.'`-in-tag rejection at calibration
> and `CreditFactorModel::validate`, plus `validate_matching_factor_ids` wired
> into `validate()` (with β = 0.0 folded-level sentinel excluded from matcher
> emission and enumeration); M3 adder vols from history for ALL modes (n−1
> estimator, unified with factor variances) — `GloballyOff` no longer zeroes
> idio vol; M4 new `FactorBumpUnit::VolPoint` (1.0 vol pt → 0.01 fractional);
> M5 matcher re-sorts `issuer_betas`; M6 unknown-issuer partial tags now error
> (no-tags PC-only fallback retained); M7 `BetaShapeMismatch` typed error;
> M8/M9 attribution mixed-currency + unknown-issuer hard errors; M10 covariance
> units contract documented; M11 non-finite sensitivity/covariance screening in
> parametric+simulation decomposers; M12 `validate_single_currency` in the delta
> engine. Moderates: `min_history` counts usable return pairs, inbound
> calibration types + `BumpSizeConfig` deny unknown fields, scale-relative
> covariance symmetry tolerance, decompose non-finite input rejection,
> Ridge/static-correlation authority + fold_ups load-bearing + tag-migration
> docs, GIL released in `calibrate`, BTreeMap→PyDict ordering, duplicate
> position_id ValueError, WASM non-finite → JS error, parity-contract inventory
> fixed. Minors: hard asserts in `SensitivityMatrix`, `UnmatchedPolicy`
> snake_case wire (legacy aliases kept), thiserror conversions, `Custom("")`
> parse rejection, doc/docstring fixes. Regression tests added across
> factor-model unit tests, `valuations/tests/credit_calibration.rs` (33),
> `credit_decomposition.rs` (13), attribution and portfolio suites; all
> verified with `mise run rust-lint`, `python-lint`, `wasm-lint` green and
> 33 Python binding tests passing after `python-build`.
> Deliberately NOT changed (open user decisions): attribution CS01 sign
> convention (attribution review OQ1); peel self-inclusion bias (MD, documented
> as model design); `decompose_period` cannot detect tag migration (documented
> contract).
>
> Original review status (2026-06-12): REVIEW COMPLETE.
> 59 findings (12 Major, 24 Moderate, 23 Minor; no Blockers). Every Major/Moderate
> finding survived an adversarial multi-verifier pass (2–3 independent refutation
> attempts per finding, majority vote); 2 candidate findings were refuted and are
> documented at the end. Severity adjudications by the verifier panel are noted
> inline on the affected findings.

## Scope and method

Reviewed surface:

- **Crate core** — all 24 `.rs` files under `finstack-quant/factor-model/{src,tests,benches}`:
  credit calibration / hierarchy / decomposition / peel, covariance, sensitivity
  matrix, config, matching (cascade / hierarchical / mapping-table / credit),
  primitives, parse, error.
- **Python bindings** — `finstack-quant-py/src/bindings/factor_model/{mod,credit}.rs`, the
  flattened risk surface in `finstack-quant-py/src/bindings/portfolio/factor_model.rs`,
  `.pyi` stubs, `parity_contract.toml` factor_model sections, and both Python test
  modules.
- **WASM bindings** — `finstack-quant-wasm/src/api/factor_model/mod.rs`,
  `exports/factor_model.js`, `index.js`/`index.d.ts` wiring, and the Rust-side WASM test.
- **Consumers** — call sites in `finstack-quant/portfolio` (delta engine, parametric
  decomposer, assignment, serialization tests), `finstack-quant/attribution`
  (credit_factor, factors), and `finstack-quant/valuations/src/correlation/factor_model.rs`
  (the separate copula factor structure; checked for semantic overlap).

Method: 7 parallel deep-read reviewers over disjoint slices, each finding then
adversarially verified by 2–3 independent agents with distinct lenses
(math re-derivation, code-context/intent, concrete reproduction), majority vote;
a completeness critic audited coverage afterwards. Primary maintainer-level reads
of `calibration.rs`, `hierarchy.rs`, `decomposition.rs`, `peel.rs`,
`covariance.rs`, `sensitivity_matrix.rs`, `config.rs`, `matching/credit.rs`,
`error.rs`, and the Python credit binding were done independently to adjudicate
disputes.

## Severity summary

| Severity | Count | Themes |
|---|---|---|
| Blocker | 0 | — |
| Major | 12 | silent zero/identity fallbacks on data gaps (anchor adder, idio vol, betas), factor-id corruption from `.` in tag values, unsorted `issuer_betas` breaking binary search, vol bump unit 100×, NaN-swallowing risk totals, raw cross-currency sums in consumers |
| Moderate | 24 | self-inclusion bias in peel regression, missing input hygiene (sortedness, finiteness at decompose time), Ridge artifact inconsistency, strict-serde gaps on inbound types, GIL held through calibration, nondeterministic dict ordering, shape-only binding tests |
| Minor | 23 | stale docs, error-type polish, wire-format inconsistencies, latent debug_assert aliasing, test self-referentiality |

### The one structural theme

The crate's dominant failure mode is **silent defaulting on partial data**: missing
as-of spreads → adder 0.0; `GloballyOff` policy → idiosyncratic vol 0.0; unknown
factor id → variance 0.0; unsorted betas → β 1.0; short beta vector → β 1.0;
unknown issuer → BucketOnly with truncated tags. Individually each fallback is
defensible; together they mean a production data gap degrades risk and attribution
*quietly* instead of loudly. A desk standard would be: every fallback either errors
under a strict mode or stamps a diagnostic the consumer can audit. This theme also
directly contradicts the crate's own design language elsewhere
(`decomposition.rs` deliberately omits absent buckets "so that callers can
distinguish 'no data' from 'data, value 0'").

## Findings

### M1 (Major) — Silent adder_at_anchor = 0.0 when a calibrated issuer is missing from asof_spreads

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:430`
**Area:** credit-calibration

**Issue.** In calibrate(), the per-issuer carry term is read with a silent zero fallback: any issuer present in history_panel.spreads but absent from inputs.asof_spreads gets adder_at_anchor = 0.0 with no error or diagnostic. Nothing in validate_calibration_inputs() requires asof_spreads to cover the calibrated universe (it only checks finiteness, lines 596-604). Worked example: issuers A (anchor spread 115) and B (220) in one bucket; if B is omitted from asof_spreads, the anchor bucket mean shifts from 113.5 to 61 and B's artifact row stores adder_at_anchor = 0.0, so reconstructing S_B = beta_pc*54 + 1*61 + 0 = 115 instead of 220 — a 105bp silent misstatement. The omission also distorts every peer's anchor bucket mean. The converse direction is also unguarded: issuers present only in asof_spreads silently enter anchor bucket means via unit betas (peel_single_observation defaults beta to 1.0) but get no row in the artifact.

**Impact.** The anchor_state/adder_at_anchor pair is the carry term in attribution (L(t) = L_anchor + delta-L). A partially populated asof_spreads map — an easy production data gap — produces a structurally valid artifact whose issuer-level carry and bucket anchors are wrong by the full idiosyncratic spread level, silently corrupting P&L attribution for the issuer and its bucket peers.

**Fix.** In validate_calibration_inputs(), require asof_spreads.keys() to be a superset (or exact match) of history_panel.spreads.keys(), or at minimum replace the unwrap_or(0.0) with a hard Error::Validation / an explicit per-issuer diagnostic flag (e.g. adder_at_anchor: Option<f64>) so consumers cannot mistake 'missing' for 'zero'.

```rust
let adder_at_anchor = anchor.adder.get(issuer_id).copied().unwrap_or(0.0);
```

---

### M2 (Major) — Tag values containing '.' silently corrupt factor IDs in the covariance and histories

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:1040`
**Area:** credit-calibration

**Issue.** Bucket paths are built by joining tag values with '.' (CreditHierarchySpec::bucket_path, hierarchy.rs:217) and then re-parsed by splitting on '.' in synth_tags_from_path() to rebuild IssuerTags for bucket_factor_id(). No validation anywhere in the crate rejects '.' inside a tag value (verified by grep: the only dot parsers are calibration.rs:811 and :1040). With hierarchy [Rating, Region] and region tag "U.S.", the level-1 bucket path is "IG.U.S."; split('.') maps rating="IG", region="U" and drops the rest, so bucket_factor_id yields "credit::level1::Rating.Region::IG.U" while the runtime matcher (matching/credit.rs:239, which uses the issuer's real tags) computes "credit::level1::Rating.Region::IG.U.S.". The calibrated covariance, static_correlation, vol_state, and factor_histories all carry the wrong factor ID; attribution-time matching finds no variance for the real factor. apply_fold_up's parent derivation via rsplit_once('.') (line 811) is similarly wrong for dotted values (diagnostics-only).

**Impact.** Realistic taxonomy values ("U.S.", "B.V.", dotted GICS-style sector codes) silently produce a covariance matrix keyed by factor IDs that the runtime matcher will never emit — risk for those buckets is dropped or mismatched with no error, since the artifact still passes validate().

**Fix.** Reject '.' in tag values (and Custom dimension keys) in validate_calibration_inputs()/build_bucket_inventory with a clear Error::Validation; structurally better: stop round-tripping through the dotted string — carry the per-level tag-value Vec alongside the joined path in run_peel so bucket_factor_id can be built without re-parsing.

```rust
let segments: Vec<&str> = path.split('.').collect();
```

---

### M3 (Major) — Idiosyncratic vol is silently 0.0 for every issuer under the default GloballyOff policy

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:1160`
**Area:** credit-calibration

**Issue.** issuer_beta_adder_vols() skips all BucketOnly issuers, so from-history adder vols are computed only for IssuerBeta issuers. Under IssuerBetaPolicy::GloballyOff — the documented default in CreditCalibrationConfig::default() (line 192) — every issuer is BucketOnly, from_history_vols is empty, the peer-proxy and global-mean stages of assign_adder_vol find nothing, and the cascade terminates at '(0.0, AdderVolSource::Default)' (line 1289). Every issuer's adder_vol_annualized is written as 0.0 and VolState.idiosyncratic gets Sample { variance: 0.0 }, even though the residual adder series exist and have nonzero variance (e.g. my traced example: issuer A's adder series [2.5, -5, 0] after bucket peel — variance dropped entirely). For buckets of N members the cross-sectional dispersion around the bucket mean is genuinely risk that lands in neither factor variances (the factor is the mean) nor idio variances (0.0).

**Impact.** Any vol forecast or VaR built from VolState systematically understates risk under the default policy: 100% of within-bucket idiosyncratic dispersion is silently zeroed. This is materially wrong portfolio risk with zero warnings — the artifact validates cleanly.

**Fix.** Compute from-history adder vols for BucketOnly issuers too (their adder_series exist in PeelOutcome and the function already guards n >= 2), or — if the IssuerBeta-only restriction is intentional — make the terminal cascade stage a hard validation error or a loud diagnostic (e.g. count of issuers assigned vol 0.0 in CalibrationDiagnostics) instead of a silent hardcoded 0.0.

```rust
if !matches!(modes.get(issuer), Some(IssuerBetaMode::IssuerBeta)) {
            continue;
        }
```

---

### M4 (Major) — vol_points default/unit contradiction yields a 100x oversized vol bump (negative vols on the down bump)

**Location:** `finstack-quant/factor-model/src/config.rs:163`
**Area:** risk-numerics

**Issue.** BumpSizeConfig documents `vol_points` as "Default volatility bump in absolute vol points" with default 1.0 (lines 163-165, 182), i.e. one vol point. But FactorBumpUnit::Absolute (line 256-257) is documented as "`0.01` = one vol point", and `to_fraction` (line 302) passes Absolute values through unchanged. Both downstream engines (finstack-quant/portfolio/src/sensitivity/delta_engine.rs:33-58 and repricing_engine.rs:141-153) feed this raw value into `mapping_to_market_bumps`, where the VolShift branch applies it as an Additive BumpUnits::Fraction shift (`value: bump_size`, delta_engine.rs:210-223, and BumpUnits::Fraction is a direct fraction per core/src/market_data/bumps.rs:107-108). So a Volatility factor under the default config gets a +/-1.00 absolute vol shift — 100 vol points — and the down-bumped market has implied vols of sigma - 1.0 (negative for any realistic surface). The default is also internally inconsistent: rates default 1.0 = 1bp = 1e-4 fractional, equity default 1.0 = 1% = 1e-2 fractional, vol default 1.0 = 1.0 fractional.

**Impact.** Any factor model containing a Volatility factor with default bump sizing produces garbage vega-style sensitivities: the central difference is a chord over a 200-vol-point range, the down leg prices off negative vols (repricing failure at best, silently wrong PV at worst), and every downstream x^T-Sigma-x risk number, Euler allocation, VaR/ES built on that column is wrong. Affects both DeltaBased and FullRepricing modes.

**Fix.** Pick one convention and enforce it: either change the default to `vol_points: 0.01` and fix the line-163 doc to say "fractional vol (0.01 = one vol point)", or introduce `FactorBumpUnit::VolPoint` whose `to_fraction` multiplies by 1e-2 and have `canonical_for(Volatility)` return it (and make the VolShift branch convert before constructing the BumpSpec). Add a regression test asserting a Volatility factor with defaults shifts a fractional vol surface by exactly +/-0.01.

```rust
/// Default volatility bump in absolute vol points.
#[serde(default = "default_one")]
pub vol_points: f64,  ... vs line 257: /// Absolute dimensionless shift — e.g. vol points (`0.01` = one vol point).
```

---

### M5 (Major) — Unsorted issuer_betas silently breaks binary search, dropping calibrated betas

**Location:** `finstack-quant/factor-model/src/matching/credit.rs:120`
**Area:** matching-primitives

**Issue.** CreditHierarchicalMatcher::lookup_row uses binary_search_by over config.issuer_betas, but the sortedness invariant (doc comment lines 53-55: "`issuer_betas` must be sorted by `issuer_id`") is never validated or enforced. CreditHierarchicalConfig is a declarative serde-deserializable config (deny_unknown_fields, "can be serialized and rebuilt"), and neither Deserialize nor CreditHierarchicalMatcher::new (lines 110-115) sorts or checks ordering. Calibration sorts its own output (credit/calibration.rs:453), but a hand-authored, merged, or tool-reordered JSON config silently violates the precondition. Binary search on an unsorted vec returns Err for issuers that ARE present, and the matcher then falls through to the unknown-issuer BucketOnly path with all betas = 1.0. Concrete trace: issuer_betas ordered ["ZETA"(pc=0.9), "ACME"(pc=1.4)]; lookup_row("ACME") probes mid=0, sees "ZETA" > "ACME", searches the empty left half, returns None; ACME's PC exposure is emitted at beta 1.0 instead of 1.4 — a ~29% understatement of its systematic credit risk, with no error or warning.

**Impact.** Silently wrong factor betas → wrong portfolio risk decomposition and credit VaR for every issuer the broken search misses. Failure is data-dependent and invisible: results look plausible (all betas 1.0) and no policy or error fires.

**Fix.** Validate sortedness (and uniqueness of issuer_id) in CreditHierarchicalMatcher::new or in CreditHierarchicalConfig deserialization/validate — e.g. check windows(2) ordering and return a config error; alternatively sort defensively on construction and reject duplicate issuer_ids.

```rust
.binary_search_by(|row| row.issuer_id.as_str().cmp(issuer_id.as_str()))
            .ok()
            .map(|idx| &self.config.issuer_betas[idx])
```

---

### M6 (Major) — Unknown-issuer tag truncation is silent and invisible to UnmatchedPolicy::Strict

**Location:** `finstack-quant/factor-model/src/matching/credit.rs:182`
**Area:** matching-primitives

**Issue.** For issuers without a calibrated row, a missing tag at any level just `break`s out of level emission: the matcher still returns Some(entries) containing at least the PC factor at beta 1.0, so the dependency counts as 'matched' and UnmatchedPolicy (even Strict, see portfolio/src/factor_model/model.rs:253) can never see the degradation. Tag lookup is exact-case (dimension_key returns lowercase "rating"/"region"/"sector"; Attributes.meta is a case-sensitive BTreeMap), and issuer lookup via ISSUER_ID_META_KEY is also exact-case (lines 139-145), so a tagging typo ("Rating" vs "rating", "acme" vs "ACME") silently collapses an issuer from PC + 3 calibrated bucket factors to a single credit::generic entry at beta 1.0. Additionally, because the matcher always returns a non-empty Some for in-scope credit dependencies, any matcher placed after it in a CascadeMatcher (matchers.rs:257-263 takes the first non-empty Some) is unreachable for credit deps — a configured fallback for unknown issuers never fires.

**Impact.** Silent risk hole: per-bucket credit exposures and calibrated betas vanish for mis-tagged or unknown issuers while the position appears fully matched. Strict unmatched policy provides false comfort; risk is understated/mis-bucketed with zero diagnostics.

**Fix.** Surface degradation: count and report 'PC-only / truncated-level' matches (e.g. a TruncatedLevels diagnostic alongside FactorMatchEntry, or a policy hook analogous to UnmatchedPolicy). At minimum, normalize tag and issuer-id keys to a canonical case at the matcher boundary, and document that cascade fallbacks after CreditHierarchical are dead for credit dependencies.

```rust
let tag_present = tags.0.contains_key(&dimension_key(dim));
            if !tag_present {
                if row.is_some() { ... }
                break;
            }
```

---

### M7 (Major) — Known issuer with short betas.levels vector silently gets beta = 1.0

**Location:** `finstack-quant/factor-model/src/matching/credit.rs:199`
**Area:** matching-primitives

**Issue.** When a calibrated row exists but row.betas.levels is shorter than hierarchy.levels (artifact/spec mismatch, possible because the config is directly deserializable and nothing validates levels.len() == hierarchy.levels.len()), the missing per-level beta silently defaults to 1.0 via `.unwrap_or(1.0)`. The matcher already has the FactorMatchError machinery for exactly this class of contract violation (MissingRequiredTag is returned for missing tags on known issuers at lines 177-181), but a truncated beta vector — the numerically analogous defect — is papered over instead of failing loud. Extra trailing betas are likewise silently ignored.

**Impact.** A malformed or version-skewed calibrated artifact produces plausible-looking but wrong level-factor loadings (1.0 instead of calibrated values), corrupting credit risk decomposition with no error, log, or policy signal.

**Fix.** For known issuers (row.is_some()), return a typed FactorMatchError (e.g. BetaVectorLengthMismatch { expected, actual }) when row.betas.levels.len() != hierarchy.levels.len(); better, validate the length invariant for every row at CreditHierarchicalMatcher::new / config-load time.

```rust
let beta = row
                .and_then(|r| r.betas.levels.get(level_idx).copied())
                .unwrap_or(1.0);
```

---

### M8 (Major) — Mixed-currency CS01 inputs are summed raw and stamped with the first position's currency

> **Cross-reference:** distinct from, but adjacent to, the 2026-06-12 attribution review's B-class `credit_factor.rs` sign-inversion finding (`:215/229/239`) and its MO-B3 currency-mislabeling finding (`dataframe.rs`). Fixing the sign question (attribution OQ1) and this raw-sum issue together is the efficient path.

**Location:** `finstack-quant/attribution/src/credit_factor.rs:162`
**Area:** consumers-integration

**Issue.** The output currency is taken from the first position only, and every position's `cs01.amount()` is then summed into shared f64 accumulators (generic_pnl_amt, level_totals, adder_total_amt) regardless of its actual currency. There is no validation that all positions share one currency. A portfolio with EUR and USD CS01s produces a numerically meaningless sum stamped as the first position's currency — implicit cross-currency math, which the workspace invariants forbid. (The empty-positions fallback to USD is also an arbitrary default.)

**Impact.** For any multi-currency credit book the attributed P&L is simply wrong (EUR and USD amounts added unit-for-unit) and mislabeled, with no error or warning — a direct violation of the currency-safety invariant in a production P&L path.

**Fix.** Validate up front that all `input.cs01.currency()` are equal and return Error::Validation otherwise (or accept an explicit target currency plus FxProvider). Reject empty `positions` instead of fabricating a USD-zero result.

```rust
let ccy = positions
    .first()
    .map(|p| p.cs01.currency())
    .unwrap_or(finstack_quant_core::currency::Currency::USD);
```

---

### M9 (Major) — Unknown issuers silently dropped from credit P&L attribution, contradicting documented hard-error contract

**Location:** `finstack-quant/attribution/src/credit_factor.rs:198`
**Area:** consumers-integration

**Issue.** The function's own doc (lines 117-127) states "For PR-7 we surface this as a hard error so misconfigurations are caught" when a position references an issuer with no IssuerBetaRow. The implementation instead emits tracing::warn! and `continue`s, dropping the position's entire credit P&L (-CS01*dS) from the attribution. This silently breaks the module-level reconciliation identity `generic_pnl + Σ levels + adder ≡ Σ -CS01·ΔS` (lines 8-12) with no machine-readable diagnostic — only a log line.

**Impact.** A stale or partially-loaded factor model (e.g. new issuance after monthly calibration) silently understates attributed credit P&L; the missing amount is invisible in the result envelope, so P&L explain vs. official P&L breaks with no audit trail of which positions were excluded.

**Fix.** Either return Error::Validation as documented, or (if skip-with-note is the desired behavior) add an `unattributed` list of dropped position_ids/amounts to CreditFactorAttribution and update the doc; do not rely on a log-only warn in a P&L path.

```rust
let Some(row) = beta_idx.get(&input.issuer_id) else {
    tracing::warn!(
        issuer_id = %input.issuer_id.as_str(),
        "Credit factor attribution skipped issuer not found in CreditFactorModel.issuer_betas"
    );
    continue;
};
```

---

### M10 (Major) — Unit contract between sensitivity deltas (per bp/%/vol-pt) and factor covariance ('factor returns') is undocumented and unvalidated

**Location:** `finstack-quant/factor-model/src/covariance.rs:7`
**Area:** consumers-integration

**Issue.** DeltaBasedEngine/FullRepricingEngine produce deltas in money per canonical bump unit — per 1bp for Rates/Credit, per 1% for Equity/FX, per vol point for Volatility (delta_engine.rs:58 divides P&L by bump_size in those units). For s'Σs to yield money², FactorCovarianceMatrix entries must therefore be covariances of factor moves in those same heterogeneous canonical units (bp² for rates, %² for equity). But the covariance doc says entries are "typically annual variance/covariance for the factor returns", which reads as fractional returns; SensitivityMatrix documents no units at all; ParametricDecomposer validates IDs/order/PSD but cannot validate units. A user supplying a fractional-rate-move covariance (e.g. 0.0064 for 80bp annual vol) against per-bp deltas understates risk by 1e8 per rates-rates entry, silently.

**Impact.** Portfolio variance/VaR/ES can be wrong by up to 8 orders of magnitude with no error; the integration tests (portfolio/tests/factor_model_engines.rs) only check delta vs DV01 and would never catch a covariance-unit mismatch.

**Fix.** Document the canonical-unit contract explicitly on FactorCovarianceMatrix, SensitivityMatrix and FactorModelConfig (rates/credit in bp, equity/commodity/FX in %, vol in vol points, matching BumpSizeConfig), and add an end-to-end test that prices a bond DV01 against a bp-units covariance and asserts the resulting vol magnitude; consider carrying a unit tag per factor axis so the decomposer can reject mismatches.

```rust
/// Entries are expected to be on a consistent variance scale, typically annual
/// variance/covariance for the factor returns used by the risk engine.
```

---

### M11 (Major) — NaN sensitivities collapse total portfolio risk to zero via NaN-swallowing max()

> **Cross-reference:** independently found by the 2026-06-12 portfolio quant review (`docs/reviews/2026-06-12-portfolio-quant-review.md`, `parametric.rs:200-208` / `simulation.rs:413`). Track remediation once, under whichever review is actioned first.

**Location:** `finstack-quant/portfolio/src/factor_model/parametric.rs:206`
**Area:** consumers-integration

**Issue.** validate_factor_axes checks the covariance matrix for finiteness (line 57) but never checks the SensitivityMatrix. If any position delta is NaN (e.g. an instrument's value_raw returns NaN under a bumped market), portfolio variance becomes NaN; `validated_variance` then evaluates `NaN < -1e-12` (false) and returns `variance.max(0.0)`. Rust's f64::max ignores NaN, so NaN variance silently becomes 0.0: `total_risk = 0` with NaN factor contributions (Variance measure) or all-zero output (Volatility/VaR/ES, since sigma=0 takes the (0.0, 0.0) branch).

**Impact.** A single bad price quietly reports the portfolio as riskless (VaR/vol = 0) instead of failing — the worst possible failure mode for a production risk engine (silent zero rather than loud error).

**Fix.** In validate_factor_axes, also reject non-finite entries in `sensitivities.as_slice()`; additionally make validated_variance return an error when `!variance.is_finite()` instead of relying on max(0.0).

```rust
if variance < -Self::VARIANCE_TOLERANCE {
    Err(finstack_quant_core::Error::Validation(format!(
        "Portfolio variance must be non-negative, got {variance}"
    )))
} else {
    Ok(variance.max(0.0))
}
```

---

### M12 (Major) — Factor sensitivity pipeline mixes native-currency PVs across positions with no FX conversion or currency check

**Location:** `finstack-quant/portfolio/src/sensitivity/delta_engine.rs:58`
**Area:** consumers-integration

**Issue.** compute_factor_column builds deltas from `instrument.value_raw(...)`, the instrument's native-currency raw PV, and FactorModel::compute_sensitivities (model.rs:306-323) feeds these rows straight into ParametricDecomposer::portfolio_exposures, which column-sums them across positions. Nothing converts to the portfolio base currency or validates that all instruments share one currency (Portfolio elsewhere supports explicit base-ccy FX rollups; position_risk.rs only says inputs are "typically the portfolio's base currency"). A portfolio holding USD and EUR bonds gets exposures that add EUR and USD DV01s unit-for-unit.

**Impact.** Risk decomposition (variance/VaR/ES and per-position Euler contributions) is wrong for any multi-currency portfolio, silently — violating the no-implicit-cross-currency invariant in the headline portfolio risk pipeline.

**Fix.** Either enforce a single-currency precondition in FactorModel::compute_sensitivities (check each instrument's base_value currency and error on mismatch), or convert per-position deltas to the portfolio base currency via an explicit FxProvider and stamp the FX policy in the result.

```rust
let pv_up = instrument.value_raw(&up_market, as_of)?;
let pv_down = instrument.value_raw(&down_market, as_of)?;
Ok((pv_up - pv_down) / (2.0 * bump_size) * *weight)
```

---

### MD1 (Moderate) — Inbound calibration input/config types do not deny unknown fields

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:213`
**Area:** credit-calibration

**Issue.** CreditCalibrationConfig (line 169), HistoryPanel (213), IssuerTagPanel (222), GenericFactorSeries (229), and CreditCalibrationInputs (238) all derive Deserialize without #[serde(deny_unknown_fields)], violating the project invariant that inbound types deny unknown fields. The same applies to VolState (hierarchy.rs:518) and FactorHistories (hierarchy.rs:542) nested inside the otherwise-strict CreditFactorModel artifact — the design note at hierarchy.rs:54-57 only justifies the omission for FactorVolModel-style enums and CalibrationDiagnostics, not these two. Stale or misspelled fields in long-lived pipeline configs (e.g. a leftover field from an older calibration schema) are silently ignored.

**Impact.** Schema drift in calibration configs and artifacts is detected late or never; a renamed-then-removed field silently stops having effect, which is exactly the failure mode the workspace's strict-serde invariant exists to prevent.

**Fix.** Add #[serde(deny_unknown_fields)] to CreditCalibrationConfig, HistoryPanel, IssuerTagPanel, GenericFactorSeries, CreditCalibrationInputs, VolState, and FactorHistories (or document an explicit forward-compat rationale per type as was done for CalibrationDiagnostics).

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryPanel {
```

---

### MD2 (Moderate) — HistoryPanel.dates sortedness/uniqueness and minimum panel length never validated

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:574`
**Area:** credit-calibration

**Issue.** HistoryPanel documents 'dates (sorted ascending)' (line 216) but validate_calibration_inputs() checks only finiteness of values; there is no check that dates are strictly increasing, duplicate-free, or that the panel has >= 2 dates in Returns mode. build_working_panel differences consecutive entries ('g.push(generic[t] - generic[t - 1])', line 680), so unsorted or duplicated dates silently produce wrong returns, wrong variances, and a wrong calibration_window; a single-date panel in Returns mode silently yields an empty working panel and an all-zero-variance model that still validates. Duplicate dates also make the as_of position lookup (line 337) ambiguous.

**Impact.** Garbage-in is accepted silently: a mis-sorted or duplicated date grid (a common upstream data bug) produces a structurally valid artifact with wrong factor variances and correlations rather than failing loud, violating the project's production-safeguard standard.

**Fix.** In validate_calibration_inputs(), enforce dates strictly increasing (windows(2) check), and require dates.len() >= 2 when use_returns_or_levels == PanelSpace::Returns (ideally a configurable minimum history).

```rust
fn validate_calibration_inputs(inputs: &CreditCalibrationInputs) -> Result<()> {
    for (idx, value) in inputs.generic_factor.values.iter().copied().enumerate() {
        validate_finite(
```

---

### MD3 (Moderate) — Self-inclusion bias in per-level beta and bucket-factor construction

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:982`
**Area:** credit-calibration

**Issue.** The level-k bucket factor is the cross-sectional mean of member residuals including the issuer's own residual (lines 949-963), and each IssuerBeta member then regresses its own residual on that mean. For a bucket of N independent issuers with equal residual variance sigma^2: Cov(r_i, mean) = sigma^2/N and Var(mean) = sigma^2/N, so the OLS beta is exactly 1 even when no common factor exists, and a spurious 'systematic' factor with variance sigma^2/N is created; with heterogeneous variances beta_i = N*sigma_i^2 / sum(sigma_j^2), mechanically inflating betas of high-vol issuers. At the default min-bucket threshold of 5 the self-weight is 20%, which is material. The model then implies fictitious within-bucket pairwise covariance ~sigma^2/N between issuers whose residuals are actually independent.

**Impact.** Beta estimates biased toward/above 1, idiosyncratic risk partially reclassified as systematic, and overstated within-bucket correlation in the factor covariance — distorting credit portfolio risk decomposition and concentration measures, worst for small buckets near the fold-up threshold.

**Fix.** Use leave-one-out bucket means as the regressor for each IssuerBeta member's OLS fit (keep the all-member mean as the published factor return for reconciliation), or at minimum document the bias and raise the default per-level threshold; a leave-one-out regressor is the standard correction for peer-mean factor construction.

```rust
let raw = ols_slope_owned(&r_series, factor_series).unwrap_or(1.0);
```

---

### MD4 (Moderate) — Adder vols use population variance (/n) while factor variances use n-1; the >=24-obs justification is not enforced

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:1170`
**Area:** credit-calibration

**Issue.** issuer_beta_adder_vols divides squared deviations by n (population variance), while factor_variances (line 1401) deliberately uses Bessel's n-1 with a doc comment arguing sparse series make the distinction material. The doc justification for /n here — 'calibration windows are required to be >= 24 observations (min_history default = 24)' (lines 1150-1152, also repeated for compute_fit_quality at 1104-1106) — is not enforced anywhere: IssuerBetaOverride::ForceIssuerBeta bypasses min_history entirely, Dynamic.min_history is caller-set with no floor, and this function only requires n >= 2 valid residuals. At n=2 the variance is understated 2x (vol by ~29%); at n=5, 20%. Residual adder series are also typically sparser than the raw spread series (None propagates through every peel stage), so effective n can be far below the level-observation count used for gating (classify_mode counts level observations, lines 642-646, not usable return pairs).

**Impact.** Idiosyncratic vols for short-history / force-included issuers are biased low, understating issuer-specific risk exactly for the names where idio risk dominates; the estimator is also inconsistent with the factor-variance estimator in the same artifact.

**Fix.** Use the unbiased n-1 estimator (the n >= 2 guard already exists) for both issuer_beta_adder_vols and compute_fit_quality's residual_std, matching factor_variances; correct or remove the inaccurate '>= 24 required' doc claims.

```rust
let var = valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / nf;
```

---

### MD5 (Moderate) — Ridge alpha makes config.covariance inconsistent with static_correlation + vol_state

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:1501`
**Area:** credit-calibration

**Issue.** Under CovarianceStrategy::Ridge, the embedded config.covariance is Sigma = D*rho*D + alpha*I, but the artifact simultaneously stores static_correlation = rho (un-ridged) and vol_state factor variances = sigma^2 (un-ridged, from factor_variances). The artifact's documented dynamic-covariance contract is Sigma(t) = D(t)*rho*D(t) (hierarchy.rs:338-339 and VolState doc at 517), so a consumer reconstructing covariance from vol_state + static_correlation gets D*rho*D — the ridge regularization silently vanishes, and the two covariance representations inside one artifact disagree on every diagonal by alpha (and on implied correlations, since Sigma's true implied correlation after the ridge is rho_ij * sigma_i*sigma_j / sqrt((sigma_i^2+alpha)(sigma_j^2+alpha)) != rho_ij). The alpha value is not recorded anywhere in the artifact, so the discrepancy is not even reconstructible.

**Impact.** Risk numbers differ depending on which artifact path a consumer uses (static config.covariance vs dynamic D(t)*rho*D(t)); near-singular sample correlations that motivated the ridge come back unregularized in the dynamic-vol path.

**Fix.** Derive static_correlation and vol_state from the final Sigma (cov2corr of D*rho*D + alpha*I, variances = sigma^2 + alpha) so all representations agree, or persist the ridge alpha in the artifact and document that dynamic consumers must re-apply it.

```rust
let mut data = d_rho_d(&stds, &rho_flat, n);
            for i in 0..n {
                data[i * n + i] += alpha;
            }
```

---

### MD6 (Moderate) — fold_ups lives in 'diagnostics' but is load-bearing model state for decomposition

**Location:** `finstack-quant/factor-model/src/credit/hierarchy.rs:593`
**Area:** credit-calibration

**Issue.** FoldUpRecord entries are documented as audit diagnostics ('Log of all fold-up events') inside CalibrationDiagnostics, which is explicitly described as for 'programmatic coverage checks' and deliberately omits deny_unknown_fields for loose evolution (lines 575-582). But decompose_levels reconstructs the folded (issuer, level) flags from model.diagnostics.fold_ups (decomposition.rs:240-252) and feeds them into peel_single_observation — they directly change bucket means, betas applied, and adders. A consumer or pipeline that strips, truncates, or regenerates diagnostics (a reasonable thing to do to 'diagnostic' data) silently changes decomposition numbers; conversely, hand-injected records alter results without tripping validate().

**Impact.** Numerical results depend on a field whose documented contract is non-load-bearing diagnostics; artifact post-processing that touches diagnostics silently breaks calibration/decomposition consistency and the reconciliation invariant.

**Fix.** Promote folded flags to first-class model state (e.g. a folded_levels: Vec<bool> or BTreeSet<usize> on IssuerBetaRow, covered by deny_unknown_fields and validate()), keep FoldUpRecord purely informational, and have decompose_levels read the first-class field.

```rust
/// Log of all fold-up events triggered by insufficient bucket coverage.
    pub fold_ups: Vec<FoldUpRecord>,
```

---

### MD7 (Moderate) — decompose_period silently violates its documented 1e-10 reconciliation invariant on tag migration or model-vintage change

**Location:** `finstack-quant/factor-model/src/credit/decomposition.rs:387`
**Area:** credit-decomposition

**Issue.** The module docs (lines 7-17) and the PeriodDecomposition docs claim the linear reconciliation invariant 'ΔS_i ≡ β_i^PC·Δgeneric + Σ_k β_i^level_k·ΔL_k + Δadder_i' holds 'to absolute tolerance 1e-10 for every issuer present in both snapshots'. That identity only holds if each issuer has identical betas, identical bucket paths (tags), and identical fold-up flags at both dates. decompose_period's only consistency check compares level counts and dimensions — it cannot detect (a) an issuer whose tags changed between snapshots (rating downgrade via runtime_tags, the most attribution-critical event in credit), or (b) snapshots produced from two different model vintages (the documented workflow is 'offline monthly calibration', so month-over-month attribution naturally straddles a recalibration). In both cases the issuer's ΔS no longer reconciles, the broken issuer is still emitted in d_adder, and there is no residual check or flag. LevelsAtDate stores no per-issuer bucket paths, betas, or fold flags, so neither decompose_period nor any downstream consumer can even detect the break.

**Impact.** Silently mis-attributed spread P&L for migrated issuers and across recalibration boundaries — the issuer's spread move leaks between the generic, level, and adder components with no error, exactly when attribution is under the most scrutiny (downgrades, month-end recalibration). Violates the module's headline documented invariant without any failure signal.

**Fix.** Stamp LevelsAtDate with enough provenance to verify the invariant: per-issuer bucket paths (or a model fingerprint such as as_of + hash of issuer_betas/fold_ups). In decompose_period, exclude (or report separately) issuers whose paths/fingerprint differ between snapshots, or return SnapshotShapeMismatch. Alternatively soften the doc claim and add an explicit reconciliation-residual output so consumers can detect leakage.

```rust
if a.level_index != b.level_index || a.dimension != b.dimension {
    return Err(DecompositionError::SnapshotShapeMismatch {
```

> **Adjudication note:** Severity adjusted Major → Moderate per adversarial-verifier majority.

---

### MD8 (Moderate) — No finite-input validation: a single NaN spread silently poisons an entire bucket cascade

**Location:** `finstack-quant/factor-model/src/credit/peel.rs:34`
**Area:** credit-decomposition

**Issue.** Neither decompose_levels (which carefully validates unknown issuers, missing tags, and beta-vector shape) nor peel_single_observation checks that observed spreads or observed_generic are finite. One NaN (or inf) spread makes that issuer's residual NaN; the residual is summed into its level-0 bucket mean (peel.rs:54-56, `entry.0 += residual`), making the broadest bucket's level value NaN; the subtraction step then makes every issuer in that bucket NaN, which cascades to every finer bucket mean and every adder under that branch. A single bad market-data tick silently NaNs out the entire rating bucket's attribution. The output structs are then serialized — serde_json renders f64::NAN as null, corrupting downstream JSON pipelines with no error anywhere.

**Impact.** One market-data glitch (NaN/inf spread) silently destroys the attribution output for a whole top-level bucket (potentially most of the book) and emits schema-breaking nulls on the wire, violating the project's fail-loud production standard.

**Fix.** In decompose_levels (and the calibration input path), validate `observed_generic.is_finite()` and every `spread.is_finite()`, returning a new DecompositionError variant (e.g. NonFiniteInput { issuer_id }) instead of propagating NaN.

```rust
let beta_pc = betas.get(issuer).map_or(1.0, |row| row.pc);
residuals.insert(issuer.clone(), spread - beta_pc * observed_generic);
```

> **Adjudication note:** Severity adjusted Major → Moderate per adversarial-verifier majority.

---

### MD9 (Moderate) — BumpSizeConfig accepts unknown fields while defaulting every field — typo'd key silently reverts bump to 1.0

**Location:** `finstack-quant/factor-model/src/config.rs:150`
**Area:** risk-numerics

**Issue.** BumpSizeConfig derives Deserialize without `#[serde(deny_unknown_fields)]`, and every field carries `#[serde(default = "default_one")]`. A config containing a misspelled key (e.g. "rates_bps" or "vol_pts") deserializes successfully: the unknown key is dropped and the intended field silently falls back to 1.0. The top-level FactorModelConfig has deny_unknown_fields (line 318), but that does not propagate to nested types. The same gap exists in the RiskMeasure manual-Deserialize helper enum (lines 120-132): `{"var":{"confidence":0.99,"horizon_days":10}}` parses with the extra field silently ignored — violating the project invariant that inbound types deny unknown fields.

**Impact.** A fat-fingered bump override in a production config is silently ignored, so finite-difference sensitivities are computed with the default bump instead of the intended one — biased deltas for nonlinear positions with no warning, and an unauditable config (what was written is not what ran).

**Fix.** Add `#[serde(deny_unknown_fields)]` to BumpSizeConfig and to the RiskMeasureSerde helper enum (it applies to struct variants).

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BumpSizeConfig {
    /// Default rates bump in basis points.
    #[serde(default = "default_one")]
    pub rates_bp: f64,
```

---

### MD10 (Moderate) — Covariance unit contract under-specified relative to per-canonical-unit sensitivities (1e8 mis-scaling trap)

**Location:** `finstack-quant/factor-model/src/covariance.rs:7`
**Area:** risk-numerics

**Issue.** The module doc says entries are "typically annual variance/covariance for the factor returns", and every test/example uses decimal return variances (0.04, 0.09). But the delta engine normalizes sensitivities by the bump in canonical units — `(pv_up - pv_down) / (2.0 * bump_size)` with bump_size in bp for rates/credit, percent for equity/FX (portfolio/src/sensitivity/delta_engine.rs:58). For x^T Sigma x in ParametricDecomposer to be P&L variance, the covariance must therefore be in squared canonical bump units per factor type (bp^2 for rates/credit, %^2 for equity/FX, absolute^2 for vol). Nothing in FactorCovarianceMatrix, FactorModelConfig (config.rs:322-323 just says "Covariance matrix aligned to factors"), or the decomposer states or validates this; a user who follows the covariance.rs doc literally for a rates factor (decimal variance 6.4e-5 for 80bp vol instead of 6400 bp^2) understates that factor's risk by 1e8, silently.

**Impact.** Silent, undetectable mis-scaling of portfolio variance/vol/VaR by up to 1e8 per asset-class block when the user-supplied covariance units do not match the per-bp / per-percent delta convention; cross-asset matrices mixing conventions corrupt every correlation cross-term.

**Fix.** Document the binding contract explicitly on FactorCovarianceMatrix and FactorModelConfig::covariance: "variance of factor moves expressed in the factor type's canonical bump unit (bp for rates/credit/inflation, percent for equity/commodity/FX, absolute vol for volatility), annualized" — with a worked rates example (80bp vol -> 6400). Consider carrying FactorBumpUnit tags on the covariance axes so the decomposer can assert unit agreement with the sensitivity engine.

```rust
/// Entries are expected to be on a consistent variance scale, typically annual
/// variance/covariance for the factor returns.
```

---

### MD11 (Moderate) — Absolute 1e-12 symmetry tolerance is not scale-invariant; rejects machine-symmetric bp^2 matrices

**Location:** `finstack-quant/factor-model/src/covariance.rs:40`
**Area:** risk-numerics

**Issue.** Symmetry validation uses an absolute tolerance: `if (lhs - rhs).abs() > 1e-12`. The unit contract is deliberately scale-agnostic ("consistent variance scale"), and the natural units implied by per-bp deltas are bp^2, where realistic entries are O(1e3-1e5) (HY credit spread vol 200bp/yr -> variance 40,000 bp^2). At magnitude 1e4 one ulp is ~1.8e-12 > 1e-12, so a matrix that is symmetric to machine precision — e.g. Sigma = D*rho*D computed with different association order for (i,j) vs (j,i), or values round-tripped through a BLAS gemm — can be spuriously rejected by a 1-ulp difference. Conversely, for tiny-scale matrices (daily decimal variances ~1e-8) the same 1e-12 is far looser than 1 ulp. ParametricDecomposer repeats the identical absolute tolerance (portfolio/src/factor_model/parametric.rs, VARIANCE_TOLERANCE = 1e-12).

**Impact.** Production risk runs with bp^2-scaled covariance can fail at config load with "Covariance matrix is not symmetric" on matrices that are bit-level fine, blocking the run; tolerance behavior silently depends on the user's unit choice.

**Fix.** Use a relative tolerance, e.g. reject when |lhs - rhs| > tol * max(1.0, |lhs|, |rhs|) with tol ~ 1e-12, or scale by max diagonal as the core pivoted Cholesky does; apply the same change to ParametricDecomposer::validate_factor_axes.

```rust
if (lhs - rhs).abs() > 1e-12 {
    return Err(finstack_quant_core::Error::Validation(format!(
        "Covariance matrix is not symmetric at ({i}, {j})"
```

---

### MD12 (Moderate) — Factor-universe validation hole for runtime BucketOnly bucket factors

**Location:** `finstack-quant/factor-model/src/matching/credit.rs:81`
**Area:** matching-primitives

**Issue.** CreditHierarchicalConfig::enumerate_factor_ids only enumerates buckets reachable from calibrated issuer_betas rows (documented in its own Limitations note). FactorModelConfig::validate_matching_factor_ids therefore passes even though at runtime an unknown issuer with full instrument tags emits bucket FactorIds (e.g. credit::level2::Rating.Region.Sector::IG.EU.TECH) that are not declared in `factors` or the covariance matrix. The downstream consumer then silently skips them: portfolio/src/factor_model/model.rs:354-360 does `let Some(factor_idx) = self.factors.iter().position(...) else { continue; }` — the sensitivity for that bucket is dropped with no diagnostic.

**Impact.** Config-load validation gives false assurance of universe alignment; at runtime, exposures to undeclared buckets are silently zeroed out of the sensitivity matrix, understating credit risk for unknown-issuer positions.

**Fix.** Either fail loud at runtime when an emitted FactorId is absent from the declared universe (error or counted warning instead of silent continue in the consumer), or restrict runtime emission to buckets enumerable from the config (drop the attributes-derived fallback buckets) so validation is complete.

```rust
/// This method only enumerates factor IDs for issuers known to the calibrated
    /// `issuer_betas`. If a runtime issuer with full tags is treated as `BucketOnly`,
    /// its bucket factor IDs are not checked here.
```

---

### MD13 (Moderate) — Bucket factor IDs can alias when tag values contain the '.' separator

**Location:** `finstack-quant/factor-model/src/matching/credit.rs:241`
**Area:** matching-primitives

**Issue.** bucket_factor_id builds `credit::level{idx}::{dim_path}::{val_path}` where val_path is CreditHierarchySpec::bucket_path joining tag values with "." (credit/hierarchy.rs:217 `parts.join(".")`). Nothing validates that tag values are free of ".". Within the same level, distinct tag tuples collide: at level 1, tags (rating="A.B", region="C") and (rating="A", region="B.C") both yield val_path "A.B.C" and therefore the identical FactorId `credit::level1::Rating.Region::A.B.C`. The level{idx} prefix protects across levels but not within a level. Plausible with custom dimensions or composite labels (e.g. "Sovereign.Agency", "BBB+.Watch"). Aliased buckets merge calibration, covariance entries, and exposure attribution for genuinely different cohorts.

**Impact.** Two distinct hierarchy buckets map to one factor: risk is attributed to the wrong cohort and bucket factor vols/correlations are blended, corrupting decomposition for affected configs without any error.

**Fix.** Reject tag values (and Custom dimension names) containing the reserved separators "." and "::" at config/calibration validation, or escape values when building val_path (e.g. percent-encode '.').

```rust
Some(FactorId::new(format!(
        "credit::level{level_idx}::{dim_path}::{val_path}"
    )))
```

---

### MD14 (Moderate) — FactorDefinition, MarketMapping, and MarketDependency lack deny_unknown_fields

**Location:** `finstack-quant/factor-model/src/primitives/definition.rs:49`
**Area:** matching-primitives

**Issue.** FactorDefinition (definition.rs:48-49) and MarketMapping (definition.rs:8-9) derive Deserialize without #[serde(deny_unknown_fields)], as does MarketDependency (dependency.rs:117). These are inbound types: FactorDefinition is nested inside FactorModelConfig (which itself denies unknown fields at config.rs:318), and MarketDependency is deserialized in portfolio assignment reports. Sibling matching types (MappingRule, FactorNode, AttributeFilter, DependencyFilter, HierarchicalConfig, CreditHierarchicalConfig, IssuerBetaRow) all carry deny_unknown_fields, so a typo in an optional field of a factor definition (e.g. "descripton") or extra fields inside a MarketMapping variant are silently ignored while the same typo in a matching rule is rejected — violating the project invariant that inbound types deny unknown fields.

**Impact.** Schema-stability invariant breach: silently ignored config fields (mis-typed optional keys, stale fields from older schemas) pass validation, producing models that differ from what the author intended with no diagnostic.

**Fix.** Add #[serde(deny_unknown_fields)] to FactorDefinition, MarketMapping, and MarketDependency (struct-variant contents), matching the convention used by the matching-config types.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorDefinition {
```

---

### MD15 (Moderate) — Assignment report swallows FactorMatchError and discards all but the deepest factor

> **Cross-reference:** the discarded-factors half of this finding also appears in the 2026-06-12 portfolio quant review (item 20, `assignment.rs:44-49`). The swallowed-`FactorMatchError` half is new here.

**Location:** `finstack-quant/portfolio/src/factor_model/assignment.rs:46`
**Area:** matching-primitives

**Issue.** assign_position_factors converts the matcher Result with `.ok()`, so a typed FactorMatchError::MissingRequiredTag (the matcher's explicit fail-loud contract violation for KNOWN issuers, matchers.rs:36-46) is silently conflated with 'no match' and routed to the unmatched list. Under the default UnmatchedPolicy::Residual (error.rs:157 asserts default is Residual) the contract violation degrades to residual treatment with at most a warn-level log. It also keeps only `next_back()` — the deepest factor — so for the credit hierarchical matcher the PC factor and all intermediate level memberships (and every beta) are absent from FactorAssignmentReport, whose doc claims to report 'Per-position matched dependencies and factor identifiers'. The sibling consumer in model.rs:348-349 correctly propagates the error via map_err, making behavior inconsistent between the audit report and the sensitivity path.

**Impact.** Audit/reporting path misrepresents multi-factor credit assignments and hides hard contract violations that the matcher deliberately raised, weakening the auditability invariant; Strict-policy users get a misleading 'unmatched dependency' message instead of the actual missing-tag error.

**Fix.** Propagate FactorMatchError as an error (or a distinct report bucket) instead of `.ok()`, and record all FactorMatchEntry items (factor_id and beta) per dependency in PositionAssignment rather than only the last entry.

```rust
.match_factor_with_betas(dependency, attributes)
            .ok()
            .flatten()
            .and_then(|entries| entries.into_iter().next_back())
```

---

### MD16 (Moderate) — CreditCalibrator.calibrate holds the GIL through the full calibration pipeline

**Location:** `finstack-quant-py/src/bindings/factor_model/credit.rs:222`
**Area:** py-bindings

**Issue.** `calibrate` runs the entire Rust calibration (panel validation, per-issuer beta regressions, bucket peeling, covariance/vol estimation over the whole history panel — finstack-quant/factor-model/src/credit/calibration.rs is 71.6K of pipeline code) inside the GIL. Every other heavy entry point in the same binding crate wraps the compute in `py.detach(...)` (e.g. parametric_var_decomposition_typed at portfolio/factor_model.rs:1697-1701, historical_var_decomposition at position_risk.rs:242), per the project rule that heavy compute releases the GIL. `decompose_levels` (per-issuer, per-level peel across the whole spread universe) has the same gap.

**Impact.** On production-size panels (thousands of issuers x years of dates) a single calibrate call blocks all Python threads for the full calibration duration — stalls notebook kernels, web workers, and any multi-threaded risk service consuming the bindings.

**Fix.** Add a `py: Python<'_>` parameter and run `py.detach(move || self.inner.calibrate(inputs))` (clone the calibrator config or restructure so the closure is 'static), mirroring the position-risk bindings. Apply the same to the decompose_levels pyfunction.

```rust
fn calibrate(&self, inputs_json: &str) -> PyResult<PyCreditFactorModel> {
    let inputs: finstack_quant_factor_model::CreditCalibrationInputs =
        serde_json::from_str(inputs_json).map_err(display_to_py)?;
    let model = self.inner.calibrate(inputs).map_err(display_to_py)?;
```

---

### MD17 (Moderate) — LevelVolContribution.by_bucket converts deterministic BTreeMap into std HashMap, leaking nondeterministic dict ordering

**Location:** `finstack-quant-py/src/bindings/portfolio/factor_model.rs:1359`
**Area:** py-bindings

**Issue.** The Rust source field is `LevelVolContribution.by_bucket: BTreeMap<String, f64>` (finstack-quant/portfolio/src/factor_model/credit_vol_forecast.rs:344), which guarantees stable sorted ordering. The binding getter collects it into `std::collections::HashMap<String, f64>` before PyO3 converts to a Python dict. Python dicts preserve insertion order, and HashMap iteration order is RandomState-seeded per process, so the bucket key order of the returned dict differs run-to-run. Every other map getter in this slice (LevelsAtDate.level_values, adder, PeriodDecomposition.level_deltas) builds the PyDict directly from BTreeMap iteration and stays deterministic; this one breaks the project's stable-ordering/determinism invariant for anyone iterating or serializing the report (e.g. JSON goldens of a CreditVolReport will be flaky).

**Impact.** Risk reports rendered or serialized from CreditVolReport.by_level[].by_bucket have nondeterministic bucket ordering across runs, breaking golden-file comparisons and audit reproducibility in a library that promises serial==parallel deterministic output.

**Fix.** Change the getter return type to `BTreeMap<String, f64>` (PyO3 converts it to a dict in sorted key order) or build the PyDict explicitly from `self.inner.by_bucket.iter()` as the credit bindings do; alternatively return `Vec<(String, f64)>` to preserve order explicitly. Update the .pyi stub accordingly.

```rust
fn by_bucket(&self) -> HashMap<String, f64> {
    self.inner
        .by_bucket
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect()
}
```

---

### MD18 (Moderate) — evaluate_risk_budget_typed silently collapses duplicate position_ids, shrinking the budget report without error

**Location:** `finstack-quant-py/src/bindings/portfolio/factor_model.rs:1788`
**Area:** py-bindings

**Issue.** Targets are built by inserting into an IndexMap keyed by PositionId; a duplicate id in `position_ids` overwrites the earlier target (and the `actual_by_id` map built inside `RiskBudget::evaluate_components` at finstack-quant/portfolio/src/factor_model/risk_budget.rs:164 collapses the same way, keeping only the last component VaR). The function validates list lengths but not uniqueness, so with duplicates the result has fewer `positions` entries than inputs and `total_overbudget`/`has_breach` are computed from a silently deduplicated subset. The legacy dict version (position_risk.rs:319-321, 340) has the same collapse and additionally mislabels `target_pct` by zipping the shortened `result.positions` against the original `target_var_pct` list.

**Impact.** A risk-budget breach report can silently drop positions and report wrong total_overbudget / has_breach=false when callers pass duplicated position ids (a common data-join error), i.e. a limit-monitoring false negative instead of a loud failure.

**Fix.** After building `targets`, check `targets.len() == n` and raise ValueError naming the duplicated id(s); apply the same guard in the legacy evaluate_risk_budget in position_risk.rs.

```rust
let mut targets: IndexMap<PositionId, f64> = IndexMap::with_capacity(n);
for (id, &pct) in shared_ids.iter().zip(target_var_pct.iter()) {
    targets.insert(id.clone(), pct);
}
```

---

### MD19 (Moderate) — Binding tests are shape/smoke-level: known-beta fixture never asserts recovered betas, reconciliation invariant untested, and the entire typed portfolio risk surface has zero Python tests

**Location:** `finstack-quant-py/tests/test_credit_factor_model_bindings.py:77`
**Area:** py-bindings

**Issue.** The fixture deliberately constructs spreads with known PC betas (`beta_pc = 0.7 + 0.05 * idx`) plus tiny noise, yet no test asserts the calibrated betas, anchor values, or factor variances against these knowns or against Rust-computed goldens — calibration tests only check counts, names, and JSON round-trips. The ΔS_i ≡ β_pc·Δgeneric + Σ β·ΔL + Δadder reconciliation invariant quoted in the PeriodDecomposition docstring is never verified per-issuer (only d_generic and the all-zero-delta case are checked). Separately, grep confirms zero Python tests reference any of the 23 typed result classes or the 4 typed functions registered in bindings/portfolio/factor_model.rs (parametric/historical_var_decomposition_typed, evaluate_risk_budget_typed, position_component_var, VolHorizon, DecompositionConfig, CreditVolReport, ...); test_factor_model_risk_bindings.py contains a single unrelated zero-factor regression test.

**Impact.** A numeric regression in calibration, decomposition wiring, or the typed binding layer (e.g. a transposed matrix, wrong default, broken getter) would pass CI; the bindings' main correctness guarantees rest entirely on Rust-side tests with no cross-language numeric parity check.

**Fix.** Add assertions recovering the constructed betas within tolerance from the calibrated artifact JSON; add a per-issuer reconciliation test (ΔS_i vs decomposed components) using two distinct spread snapshots; add at least one numeric test per typed function (e.g. 2x2 covariance where component VaRs are hand-computable, sum(component_var)==portfolio_var) and smoke tests for VolHorizon/DecompositionConfig/position_component_var (including its KeyError path).

```rust
beta_pc = 0.7 + 0.05 * idx
series: list[float | None] = [
    base + beta_pc * (generic_values[i] - 100.0) + 0.1 * math.cos(idx + i * 0.5) for i in range(n)
]
```

---

### MD20 (Moderate) — Non-finite f64 crosses the WASM boundary unchecked and is silently serialized as JSON null in risk outputs

**Location:** `finstack-quant-wasm/src/api/factor_model/mod.rs:181`
**Area:** wasm-bindings

**Issue.** decompose_levels accepts `observed_generic: f64` with no finiteness guard, and neither core `finstack_quant_factor_model::decompose_levels` (finstack-quant/factor-model/src/credit/decomposition.rs:216-222) nor `peel_single_observation` (finstack-quant/factor-model/src/credit/peel.rs:34, `spread - beta_pc * observed_generic`) validates it. A JS caller passing NaN (e.g. the result of a failed `Number(...)` parse) gets a fully 'successful' LevelsAtDate whose generic, every bucket value, and every issuer adder are NaN. All toJson methods in this module (`WasmLevelsAtDate::to_json` line 130-132, `WasmPeriodDecomposition::to_json` line 153-155, `WasmFactorCovarianceForecast::covariance_at` line 273) use serde_json, which serializes NaN/inf as `null` — so the JS consumer receives nulls in a spread-attribution/covariance payload with no error thrown. The same hole exists in the vol path: the variance guard in finstack-quant/portfolio/src/factor_model/credit_vol_forecast.rs:195 (`if variance < 0.0`) is false for NaN, so a NaN calibrated variance flows through sqrt to a null covariance entry. The mirror Python binding (finstack-quant-py/src/bindings/factor_model/credit.rs:485) has the identical gap, so fixing only the wasm layer would create parity drift — the guard belongs in core decompose_levels.

**Impact.** Silent corruption of credit spread-attribution and factor-covariance results consumed in the browser/Node: nulls appear where risk numbers should be, downstream JS aggregation either throws far from the source or coerces null to 0, understating bucket moves and idiosyncratic risk with no audit trail. Violates the project's fail-loud production standard.

**Fix.** Reject non-finite inputs at the source: add a finiteness check on `observed_generic` (and ideally on parsed spread values) inside core `decompose_levels`, returning a new `DecompositionError::NonFiniteInput` variant; both bindings then fail loud automatically. Additionally consider a non-finite scan before serde_json serialization in the wasm toJson/covarianceAt paths so NaN produced by any future path can never degrade to `null`.

```rust
pub fn decompose_levels(
    model: &WasmCreditFactorModel,
    observed_spreads_json: &str,
    observed_generic: f64,
```

> **Adjudication note:** Severity adjusted Major → Moderate per adversarial-verifier majority.

---

### MD21 (Moderate) — WASM test suite never executes the decompose/forecast wrappers and pins no numeric values

**Location:** `finstack-quant-wasm/tests/wasm_credit_factor_hierarchy.rs:111`
**Area:** wasm-bindings

**Issue.** The wasm-bindgen test named `calibrate_then_decompose_round_trip` (lines 110-133) only calls `WasmCreditCalibrator::new`, `calibrate`, and `to_json`, then asserts the presence of `schema_version` — it never calls `decompose_levels`, `decompose_period`, or any `WasmFactorCovarianceForecast` method despite its name. The native tests in finstack-quant-wasm/src/api/factor_model/mod.rs (lines 464-510) call `finstack_quant_factor_model::decompose_levels` directly, bypassing the wrapper's JSON parsing wiring (observed_spreads_json/runtime_tags_json deserialization at mod.rs:185-198, date parsing at mod.rs:188), and the comment justifying this (mod.rs:317-320, js_sys only on wasm32) does not hold for success paths, which never construct a JsValue. There is also no facade test for the factor_model namespace under finstack-quant-wasm/tests/facade/ (only cashflows, core_namespace, plain_object_returns exist). Net result: 5 of 9 public wasm entry points in this module (`decomposeLevels`, `decomposePeriod`, `covarianceAt`, `idiosyncraticVol`, `factorModelAt`) have zero execution coverage on any target, and no test on either target pins a numeric output value (only structure and schema_version are asserted).

**Impact.** A wiring regression in the wrappers — swapped from/to arguments in decomposePeriod (which would flip the sign of every reported delta), a wrong key type in the observed-spreads map parse, or a horizon mis-route — would ship undetected. Numeric drift between the wasm surface and the Rust crate would also go unnoticed since nothing compares values.

**Fix.** Extend the wasm-bindgen test to drive the full pipeline through the wrappers (calibrate → decomposeLevels at t0/t1 → decomposePeriod → covarianceAt/idiosyncraticVol), asserting pinned numeric values that match the native fixture (e.g. the unit-beta reconciliation: with spreads {A:150, B:175}, generic 100, rating bucket mean 62.5 and adders 0; d_generic 0.5 between snapshots). Also add native success-path tests that call the `super::decompose_levels`/`decompose_period` wrappers directly to cover the JSON parsing code, and a facade .mjs test asserting `factor_model.credit.decomposeLevels` is a function.

```rust
fn calibrate_then_decompose_round_trip() {
    let config_json = minimal_config_json();
    let inputs_json = minimal_inputs_json();
```

---

### MD22 (Moderate) — Inbound CreditCalibrationConfig/CreditCalibrationInputs accept unknown JSON fields, violating the strict-serde invariant at the WASM (and Python) boundary

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:169`
**Area:** wasm-bindings

**Issue.** `WasmCreditCalibrator::new` (finstack-quant-wasm/src/api/factor_model/mod.rs:89-95) and `calibrate` (mod.rs:103-108) deserialize user-supplied JSON into `CreditCalibrationConfig` and `CreditCalibrationInputs`, but neither struct carries `#[serde(deny_unknown_fields)]` (calibration.rs:169-170 and 238-239; `grep deny_unknown_fields calibration.rs` is empty). The workspace invariant is 'inbound types deny unknown fields', and the hierarchy.rs design note (lines 54-57) documents the forward-compat exception only for artifact sub-types like FactorVolModel/CalibrationDiagnostics — not for the calibration config/inputs, whose advertised contract in both bindings is 'pass plain dicts serialized via JSON'. A misplaced or misspelled extra key (e.g. nesting a shrinkage parameter at the top level, or carrying a key from a newer schema) is silently dropped instead of rejected.

**Impact.** Calibration silently runs with different parameters than the operator believes were supplied (e.g. an ignored override key), producing plausible-but-wrong factor vols and betas with no error and no audit trail. Schema drift between config writers and this reader is undetectable.

**Fix.** Add `#[serde(deny_unknown_fields)]` to `CreditCalibrationConfig`, `CreditCalibrationInputs`, `HistoryPanel`, `IssuerTagPanel`, and `GenericFactorSeries` (all inbound through the binding JSON-string API), keeping the documented exceptions only on forward-extensible artifact sub-types.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditCalibrationConfig {
```

---

### MD23 (Moderate) — Issuers missing from period.d_adder get silent partial attribution, breaking the documented reconciliation identity

**Location:** `finstack-quant/attribution/src/credit_factor.rs:237`
**Area:** consumers-integration

**Issue.** decompose_period restricts d_adder (and bucket deltas) to issuers/buckets present in BOTH snapshots (factor-model/src/credit/decomposition.rs:93-95: "restricted to issuers present in both snapshots"). For a position whose issuer is in the model but missing from observed_spreads at one snapshot date, this code still books generic and level contributions but the adder term is silently zero, so the position's attributed P&L no longer equals -CS01·ΔS. The in-code comment at lines 219-227 ("the contribution falls into the adder via the period's d_adder") is incorrect for these one-sided cases — nothing absorbs the omitted term, and the unused `delta_spread` field that could sanity-check this is explicitly informational.

**Impact.** Data gaps (issuer dropped from the spread feed, defaults, new issuance mid-period) cause partial, non-reconciling attribution with no diagnostic — the module's headline 1e-8 reconciliation guarantee fails silently in exactly the messy-data situations where users rely on it.

**Fix.** When an issuer with non-zero CS01 has a row in the model but no entry in period.d_adder, emit a structured diagnostic (or error under a strict option), and/or use the stored `delta_spread` to assert per-position reconciliation within tolerance.

```rust
// Adder contribution. Issuers absent from period.d_adder contribute 0.
if let Some(d_adder) = period.d_adder.get(&input.issuer_id) {
```

---

### MD24 (Moderate) — Single/Two-factor constructors silently clamp negative or out-of-range vols and correlation that the Multi-factor path explicitly rejects

**Location:** `finstack-quant/valuations/src/correlation/factor_model.rs:381`
**Area:** consumers-integration

**Issue.** LatentSingleFactor::new and LatentTwoFactor::new silently clamp volatility to [0.01, 2.0] and correlation to [-0.99, 0.99], including via the serde-loadable LatentFactorSpec::build() path — so a config with prepay_vol = -0.2 (sign-flip bug) silently prices with 0.01, and vol = 3.5 silently becomes 2.0. The same file's validated() (lines 701-714) explicitly rejects negative/non-finite vols with the rationale "Silent clamping of `-0.2` to `0.01` would mask sign-flip bugs upstream", and uncorrelated() (lines 749-758) still silently replaces a wrong-length vol vector with unit vols (reachable from new_or_identity's last-resort fallback). Validation strictness is inconsistent across the three constructors of the same enum, and the upper clamp differs too (2.0 vs 10.0).

**Impact.** Structured-credit stochastic pricing (the production consumer of these specs) can run with materially different vol/correlation than the user configured, with no error — misconfiguration is masked exactly as the multi-factor comment warns.

**Fix.** Make Single/Two constructors (or at least LatentFactorSpec::build) return Result and reject non-finite, negative, or out-of-range inputs like LatentMultiFactor::validated does; align the clamp bounds; have new_or_identity propagate VolatilityLengthMismatch instead of falling back to unit vols.

```rust
pub fn new(volatility: f64, mean_reversion: f64) -> Self {
    let vol = volatility.clamp(0.01, 2.0);
```

---

### MN1 (Minor) — FactorCorrelationMatrix validation never checks off-diagonals are in [-1, 1] (nor PSD)

**Location:** `finstack-quant/factor-model/src/credit/hierarchy.rs:436`
**Area:** credit-calibration

**Issue.** Both FactorCorrelationMatrix::new and check_structure validate shape, unit diagonal, symmetry, and duplicate IDs only. An inbound JSON artifact with off-diagonal entries of 5.0 (or NaN-free but wildly out-of-range values) passes CreditFactorModel::validate(), because check_structure has no bounds check and no PSD check — unlike finstack_quant_analytics::validate_correlation_matrix which checks both. The calibrator's own output is clamped to [-1,1] and PSD-repaired, so this gap only bites on deserialized/hand-assembled artifacts — exactly the path validate() exists to guard (the doc at lines 423-427 says it exists to catch matrices 'constructed via direct field assignment').

**Impact.** A corrupted or hand-edited risk artifact with a mathematically invalid correlation matrix is accepted and flows into Sigma(t) = D(t)*rho*D(t), producing nonsensical or negative portfolio variances downstream with no error at load time.

**Fix.** Add an off-diagonal |rho_ij| <= 1 + 1e-9 check to check_structure/new (cheap and unambiguous), and consider delegating the full check (including the Cholesky PSD test) to finstack_quant_analytics::validate_correlation_matrix on the flattened data.

```rust
pub fn check_structure(&self) -> finstack_quant_core::Result<()> {
```

> **Adjudication note:** Severity adjusted Moderate → Minor per adversarial-verifier majority.

---

### MN2 (Minor) — variance/covariance/correlation accessors silently return 0.0 for unknown factor IDs

**Location:** `finstack-quant/factor-model/src/covariance.rs:83`
**Area:** risk-numerics

**Issue.** `variance`, `covariance`, and `correlation` all return 0.0 when a FactorId is not in the index (`let Some(&idx) = self.index.get(factor) else { return 0.0; };`, repeated at lines 92-97). A typo'd or stale factor ID is indistinguishable from a genuinely zero-variance factor. No production code currently calls these (the ParametricDecomposer uses `as_slice()` with strict order validation, parametric.rs:47), but they are public crate API exported at the root, advertised in the docs, and exercised in benches — the first downstream consumer that aggregates risk by ID inherits a silent-zero-risk failure mode, in direct tension with the fail-loud philosophy used everywhere else in this pipeline (the engine validates axes, dimensions, finiteness, and PSD loudly).

**Impact.** Latent silent under-statement of risk: any future ID-keyed consumer (bindings, what-if tooling, reporting) drops the variance contribution of misspelled/renamed factors to zero with no error, producing too-small portfolio vol/VaR.

**Fix.** Return `Option<f64>` (or `finstack_quant_core::Result` with FactorModelError::MissingFactor) from the ID-keyed accessors; if a 0.0-defaulting convenience is genuinely needed, name it explicitly (e.g. `variance_or_zero`).

```rust
let Some(&idx) = self.index.get(factor) else {
    return 0.0;
};
```

> **Adjudication note:** Severity adjusted Moderate → Minor per adversarial-verifier majority.

---

### MN3 (Minor) — delta/set_delta bounds checks are debug_assert-only: out-of-range factor index silently aliases into the next position's row in release builds

**Location:** `finstack-quant/factor-model/src/sensitivity_matrix.rs:68`
**Area:** risk-numerics

**Issue.** `delta` and `set_delta` guard indices with `debug_assert!` and then index raw row-major storage: `self.data[position_idx * self.n_factors + factor_idx]`. In release builds the asserts compile out. An out-of-range `position_idx` still panics via slice bounds, but an out-of-range `factor_idx` (n_factors <= factor_idx, with position_idx small) computes an offset that is still inside `data` and silently reads/writes an element belonging to a different position — no panic, no error, just a wrong sensitivity. Current callers derive indices by enumeration (delta_engine.rs:245-248) so the hazard is latent, but this is the canonical public matrix type for the risk pipeline and the slice's own framing applies: index misalignment here is a P&L bug, and this design converts a loud caller bug into a silent one.

**Impact.** A future caller that maps factor IDs to indices against the wrong axis ordering gets sensitivities attributed to the wrong position/factor with no failure signal — corrupted risk decompositions that pass all runtime checks.

**Fix.** Promote the guards to release-mode checks: use `assert!` (cost is negligible next to repricing) or return Option/Result from `delta`/`set_delta`; alternatively compute the index via a checked helper that verifies factor_idx < n_factors before forming the flat offset.

```rust
debug_assert!(
    factor_idx < self.n_factors,
    ...
);
self.data[position_idx * self.n_factors + factor_idx]
```

> **Adjudication note:** Verifier panel split 1–1; adjudicated Minor on first-hand read: the aliasing arithmetic is real in release builds, but every workspace caller derives indices by enumerating the matrix’s own axes, so no reachable path triggers it today. Worth hardening cheaply (promote to hard assert!/Result) since the type is a public crate-root export.

---

### MN4 (Minor) — FactorType::from_str silently parses 'Custom-Weather' / 'custom weather' as Custom("")

**Location:** `finstack-quant/factor-model/src/primitives/factor_types.rs:74`
**Area:** matching-primitives

**Issue.** The custom-prefix detection runs on the normalized label (normalize_label maps '-', '/', ' ' to '*'), so "Custom-Weather" and "custom weather" both normalize to "custom_weather" and pass the starts_with("custom*") gate. But the name is then extracted from the ORIGINAL string using only `split_once(':')` or `split_once('_')` — neither matches '-' or ' ' — so `.map(...).unwrap_or("")` yields Custom(""). The normalizer declares these separators equivalent while the extractor only understands two of them, silently producing an empty-named custom factor type instead of Custom("Weather") or an error. "custom:" with no name also yields Custom("") rather than rejecting.

**Impact.** String-based config/binding ingestion mis-classifies factor types without error; FactorType drives per-type bump-size selection downstream (BumpSizeConfig::bump_size_for_factor), so a mangled Custom("") can change which bump override applies.

**Fix.** Extract the name from the normalized string (split on the first '*' or ':' of the normalized form, preserving original casing via index arithmetic), or split the original on any of [':','*','-',' ','/']; reject empty custom names with an error.

```rust
let name = s
                .split_once(':')
                .or_else(|| s.split_once('_'))
                .map(|(_, v)| v.trim())
                .unwrap_or("");
```

> **Adjudication note:** Severity adjusted Moderate → Minor per adversarial-verifier majority.

---

### MN5 (Minor) — Dynamic min_history gates on level observations, not usable return pairs

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:644`
**Area:** credit-calibration

**Issue.** classify_mode counts raw Some() entries in the level-space spread series ('s.iter().filter(|v| v.is_some()).count()'), but in the default PanelSpace::Returns mode the OLS estimator consumes consecutive-pair differences (build_working_panel requires Some at both t-1 and t). An issuer observed on 24 alternating dates passes min_history = 24 yet contributes zero return observations; ols_slope then silently falls back to beta = 1.0 (line 890, '.unwrap_or(1.0)') with mode still recorded as IssuerBeta and fit_quality = None, blurring the IssuerBeta/BucketOnly distinction in the artifact.

**Impact.** Issuers can be classified IssuerBeta on a gating metric the estimator never sees, producing rows labeled IssuerBeta whose betas are actually un-fitted defaults — a mild auditability and classification-consistency gap rather than a numeric error.

**Fix.** In Returns mode, count valid consecutive pairs (windows of 2 with both Some) against min_history, or record an explicit fallback marker (e.g. fit_quality with n_obs = 0 or a dedicated AdderVolSource-style provenance enum for betas) when OLS falls back to 1.0.

```rust
.map(|s| s.iter().filter(|v| v.is_some()).count())
```

---

### MN6 (Minor) — Stale 'Marginal-mean limitation' doc contradicts the pairwise-overlap implementation

**Location:** `finstack-quant/factor-model/src/credit/calibration.rs:1561`
**Area:** credit-calibration

**Issue.** The doc comment on sample_correlation_flat still describes the old behavior — 'The mean subtracted before computing the covariance for each factor is its **marginal** mean … a deliberate simplification' — but the implementation (lines 1616-1645) and its regression test (sample_correlation_uses_pairwise_overlap_mean_on_sparse_panel, line 1799) now compute proper pairwise-overlap means. A reader auditing the estimator from the docs will conclude the entries are not Pearson correlations when they are.

**Impact.** Misleading documentation on a risk-critical estimator; wastes reviewer/auditor time and could prompt an unnecessary 'fix' that reintroduces the original bug.

**Fix.** Delete the '# Marginal-mean limitation' section and replace it with a sentence stating that per-pair overlap means are used (pairwise-complete Pearson correlation), keeping the existing PSD-guarantee caveat.

```rust
/// The mean subtracted before computing the covariance for each factor is its
/// **marginal** mean — the mean over all dates where that factor individually
```

---

### MN7 (Minor) — DateMismatchInPeriod field doc comments are swapped

**Location:** `finstack-quant/factor-model/src/credit/decomposition.rs:143`
**Area:** credit-decomposition

**Issue.** The error fires when from.date > to.date, so `from` is the later date (supplied first) and `to` is the earlier date (supplied second). The doc comments say the opposite: `from` is documented as 'Earlier-but-supplied-second date.' and `to` as 'Later-but-supplied-first date.'

**Impact.** Misleading API docs for an error users will hit when wiring period attribution; trivial but confusing during incident triage.

**Fix.** Swap the two doc comments: `from` is 'Later date supplied as the from-snapshot', `to` is 'Earlier date supplied as the to-snapshot'.

```rust
DateMismatchInPeriod {
    /// Earlier-but-supplied-second date.
    from: Date,
    /// Later-but-supplied-first date.
    to: Date,
```

---

### MN8 (Minor) — runtime_tags silently ignored for model-resident issuers; precedence undocumented

**Location:** `finstack-quant/factor-model/src/credit/decomposition.rs:269`
**Area:** credit-decomposition

**Issue.** Resolution checks the model beta index first and only falls back to runtime_tags when the issuer is absent from the model. A caller who supplies updated tags for a model-resident issuer (e.g. to reflect a downgrade after calibration) has them silently ignored — decomposition proceeds under the stale calibration-vintage tags with no warning. This precedence is arguably the safer choice (it keeps bucket paths stable so the period invariant holds), but the §5.4 doc block (lines 195-201) only describes the absent-from-model case and never states that model tags win over runtime_tags.

**Impact.** Users can reasonably believe they have repointed a downgraded issuer to its new bucket when they have not; the surprise compounds the migration blind spot in decompose_period.

**Fix.** Document the model-tags-win precedence explicitly in the decompose_levels doc (and in the Python/WASM binding docstrings), or return an error when runtime_tags conflicts with a model-resident issuer's tags.

```rust
if let Some(row) = beta_idx.get(issuer) {
    resolved.insert(
        issuer,
        Resolved {
            betas: &row.betas,
            tags: &row.tags,
```

---

### MN9 (Minor) — is_psd swallows Cholesky error detail; NaN inputs reported as 'not positive semi-definite'

**Location:** `finstack-quant/factor-model/src/covariance.rs:134`
**Area:** risk-numerics

**Issue.** NaN off-diagonal pairs pass the symmetry check at line 40 because `(NaN - NaN).abs() > 1e-12` is false. They are then caught by `cholesky_correlation`, which correctly rejects non-finite input (CholeskyError::NonFiniteInput, core/src/math/linalg.rs:324-331) — so invalid data cannot enter the matrix (clean) — but `is_psd` collapses the typed error to a bool, so NonFiniteInput, DimensionMismatch, and NotPositiveDefinite all surface as the misleading "Covariance matrix is not positive semi-definite". The crate even defines FactorModelError::InvalidCovariance { reason } for exactly this, and never uses it.

**Impact.** Risk teams debugging a rejected covariance get a wrong diagnosis (chasing PSD/regularization when the actual problem is a NaN from an upstream join), lengthening incident resolution.

**Fix.** Have is_psd return the CholeskyError and interpolate it into the Validation message (as ParametricDecomposer already does via `cholesky(data, n).map_err(...)`), and/or add an explicit `is_finite` screen before the symmetry loop so NaN is reported as a non-finite-entry error with its (row, col).

```rust
fn is_psd(data: &[f64], n: usize) -> bool {
    finstack_quant_core::math::linalg::cholesky_correlation(data, n).is_ok()
}
```

---

### MN10 (Minor) — FactorCovarianceMatrix deserialize helper does not deny unknown fields

**Location:** `finstack-quant/factor-model/src/covariance.rs:145`
**Area:** risk-numerics

**Issue.** The inline `FactorCovarianceMatrixSerde` helper derives Deserialize without `#[serde(deny_unknown_fields)]`, so inbound JSON with extra keys (e.g. a stale "correlations" or misspelled "factorIds" alongside the correct keys) is silently accepted. The structural checks (n, n*n, PSD) still run, so this cannot corrupt values — it only violates the project's strict-serde invariant for inbound types.

**Impact.** Schema drift and typos in persisted covariance payloads go unnoticed instead of failing fast, weakening the long-lived-pipeline guarantees the project advertises.

**Fix.** Add `#[serde(deny_unknown_fields)]` to the FactorCovarianceMatrixSerde helper struct.

```rust
#[derive(Deserialize)]
struct FactorCovarianceMatrixSerde {
    factor_ids: Vec<FactorId>,
    n: usize,
    data: Vec<f64>,
}
```

---

### MN11 (Minor) — PSD rejection property is false at the small-variance corner (latent flaky proptest; marginally indefinite matrices accepted)

**Location:** `finstack-quant/factor-model/src/covariance.rs:205`
**Area:** risk-numerics

**Issue.** `two_factor_covariance_rejects_out_of_bounds_correlation` asserts every matrix with |corr| > 1 is rejected, over variances down to 1e-6 and excess down to 1e-6. But cholesky_correlation's pivot tolerance has an absolute floor: `tol = 1e-10 * max(max_diag.abs(), 1.0)` (linalg.rs:337), so for max_diag = 1e-6 the tolerance is 1e-10 absolute. With variance_a = variance_b = 1e-6, excess = 1e-6, the second pivot is ~ -2 *excess* variance ~ -2e-12, which is > -1e-10 and is truncated as semidefinite — the matrix is *accepted* and the property fails. The corner has tiny sampling probability under 96 uniform cases, so the test currently passes by luck.

**Impact.** A latent intermittent CI failure, and a documented-by-test guarantee ("out-of-bounds correlation always rejected") that does not actually hold for small-scale covariance matrices; marginally indefinite matrices (negative eigenvalue above -1e-10 absolute) validate as PSD regardless of matrix scale.

**Fix.** Constrain the proptest domain so the indefiniteness exceeds the acceptance tolerance (e.g. require excess * min_variance > 1e-9), or assert the precise contract (rejected OR truncated-rank accepted); longer term, consider scaling the pivot floor by max_diag without the max(.., 1.0) clamp for covariance-mode inputs.

```rust
let covariance = (1.0 + excess) * (variance_a * variance_b).sqrt();
...
prop_assert!(result.is_err());
```

---

### MN12 (Minor) — FactorModelError is mostly dead, hand-rolled instead of thiserror, and not #[non_exhaustive]

**Location:** `finstack-quant/factor-model/src/error.rs:8`
**Area:** risk-numerics

**Issue.** Of the five variants, only UnmatchedDependency is ever constructed (once, to format a message string in portfolio/src/factor_model/model.rs:255). MissingFactor, InvalidCovariance, RepricingFailed, and AmbiguousMatch are dead — covariance validation returns stringly-typed `finstack_quant_core::Error::Validation` instead of InvalidCovariance, and matching/repricing paths use their own error types. The enum also hand-implements Display/Error rather than using thiserror (the project's stated standard for library crates) and lacks `#[non_exhaustive]` despite being a public, growable error enum.

**Impact.** Misleading public API surface: consumers writing match arms against these variants will never see them fire; error-handling code paths are untestable dead weight; adding a variant later is a semver break without non_exhaustive.

**Fix.** Either wire the variants into the actual failure paths (use InvalidCovariance in FactorCovarianceMatrix::new, MissingFactor for ID-keyed lookups) or delete the dead variants; convert to `#[derive(thiserror::Error)]` and add `#[non_exhaustive]`.

```rust
#[derive(Debug)]
pub enum FactorModelError {
    /// No factor matched a dependency for a position.
    UnmatchedDependency {
```

---

### MN13 (Minor) — UnmatchedPolicy serde wire format is PascalCase, inconsistent with its own Display/FromStr and the crate's snake_case convention

**Location:** `finstack-quant/factor-model/src/error.rs:89`
**Area:** risk-numerics

**Issue.** UnmatchedPolicy derives Serialize/Deserialize with no `rename_all`, so it serializes as "Strict"/"Residual"/"Warn". Its Display/FromStr produce and accept "strict"/"residual"/"warn", and every other enum in the config surface (PricingMode, RiskMeasure, FactorBumpUnit) uses `#[serde(rename_all = "snake_case")]`. A user who writes `"unmatched_policy": "strict"` in JSON — the form the crate itself prints — gets a deserialization error.

**Impact.** Confusing config authoring experience and an inconsistent wire schema that is awkward to change later without a breaking migration; the longer it ships, the more persisted configs lock in the PascalCase names.

**Fix.** Add `#[serde(rename_all = "snake_case")]` to UnmatchedPolicy now (before the schema calcifies), with a migration note; alternatively accept both via alias attributes during a deprecation window.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UnmatchedPolicy {
    /// Fail immediately when any dependency is unmatched.
```

---

### MN14 (Minor) — FactorMatchError hand-rolls Display/Error instead of using thiserror

**Location:** `finstack-quant/factor-model/src/matching/matchers.rs:40`
**Area:** matching-primitives

**Issue.** FactorMatchError implements Display and std::error::Error manually (lines 48-59) rather than deriving via thiserror, contrary to the workspace error-handling standard that library crates use thiserror-based Error enums. It is also not #[non_exhaustive], so adding the variants suggested above (e.g. beta-length mismatch) is a breaking change for downstream matchers.

**Impact.** Inconsistent with crate-wide error conventions; future variant additions break downstream exhaustive matches (see the exhaustive match in credit.rs tests line 403-407).

**Fix.** Derive with #[derive(thiserror::Error)] and mark the enum #[non_exhaustive].

```rust
pub enum FactorMatchError {
    /// A required issuer tag is missing for hierarchy bucketing.
    MissingRequiredTag {
```

---

### MN15 (Minor) — Parse failures discard the offending label and helper message

**Location:** `finstack-quant/factor-model/src/primitives/factor_types.rs:86`
**Area:** matching-primitives

**Issue.** FactorType::from_str maps any unrecognized label to the context-free `finstack_quant_core::InputError::Invalid`, and CurveType/DependencyType::from_str (dependency.rs:50-52, 110-113) likewise drop the `Err(String)` from parse_normalized_enum, which already carries a useful "unknown variant '{key}'" message (parse.rs:34).

**Impact.** Config-ingestion errors report only a generic 'Invalid' with no indication of which label or which enum failed, hurting diagnosability of malformed ids in larger configs.

**Fix.** Map to an error variant that carries the input string (e.g. InputError::InvalidValue { field, value } or equivalent), threading through the message parse_normalized_enum already builds.

```rust
_ => Err(finstack_quant_core::InputError::Invalid.into()),
```

---

### MN16 (Minor) — factor_model module inventory in parity contract does not match the Rust crate tree

**Location:** `finstack-quant-py/parity_contract.toml:90`
**Area:** py-bindings

**Issue.** `[crates.factor_model.modules]` tracks `primitives`, `matching`, `credit`, and `calibration`. The actual top-level public modules of finstack-quant-factor-model (finstack-quant/factor-model/src/lib.rs:10-23) are `config`, `covariance`, `credit`, `error`, `matching`, `primitives`, `sensitivity_matrix` — there is no top-level `calibration` module (calibration lives under `credit::calibration`), and `config`, `covariance`, `error`, `sensitivity_matrix` are untracked. The Python-side claims are correct (only `credit` is registered in bindings/factor_model/mod.rs, matching status="exists"; the topology test only validates importability of python paths), so this is a documentation-of-record gap on the Rust axis, not a failing test.

**Impact.** The contract is the audit document for binding coverage decisions; reviewers consulting it will conclude RiskMeasure/FactorCovarianceMatrix/SensitivityMatrix surfaces were never considered for binding rather than deliberately deferred, and the phantom `calibration` entry can never transition to exists.

**Fix.** Replace the `calibration` row with `credit.calibration` context inside the credit note (or drop it), and add rows for `config`, `covariance`, `error`, `sensitivity_matrix` with status="missing" and deferral notes mirroring the existing style.

```rust
calibration = { python = "finstack_quant.factor_model.calibration", status = "missing", note = "Generic FactorCalibrator is a Rust trait and not directly host-language constructible." }
```

---

### MN17 (Minor) — Calibrator errors mapped via display_to_py instead of core_to_py, flattening the documented exception taxonomy

**Location:** `finstack-quant-py/src/bindings/factor_model/credit.rs:225`
**Area:** py-bindings

**Issue.** `CreditCalibrator::calibrate` returns `finstack_quant_core::Result` (calibration.rs:297, `use finstack_quant_core::{Error, Result}` at line 72), but the binding maps failures with `display_to_py`, which is unconditionally `PyValueError` (errors.rs:178). The project's error-mapping convention (errors.rs:75-79 and the binding standards) routes `Error::Calibration`/`Error::Internal`/solver failures to RuntimeError and missing-id lookups to KeyError via `core_to_py`. Today calibrate only emits `Error::Validation` so behavior coincides, but the mapping is latent drift: any future Calibration/Internal variant from this path will surface as ValueError instead of RuntimeError, diverging from every other finstack_quant_core::Error site in the bindings.

**Impact.** Exception-type contract divergence between domains; downstream retry/alerting logic keyed on RuntimeError-vs-ValueError would misclassify calibration failures if the Rust error surface grows.

**Fix.** Map `self.inner.calibrate(inputs)` errors with `crate::errors::core_to_py` (keep display_to_py for the serde_json parse step and for the non-core DecompositionError/ValuationsError types).

```rust
let model = self.inner.calibrate(inputs).map_err(display_to_py)?;
```

---

### MN18 (Minor) — decompose_levels docstring example passes a Python dict where the binding requires a JSON string; Args names don't match parameters

**Location:** `finstack-quant-py/src/bindings/factor_model/credit.rs:479`
**Area:** py-bindings

**Issue.** The pyfunction signature is `(model, observed_spreads_json, observed_generic, as_of, runtime_tags_json=None)` with `observed_spreads_json: &str`, but the docstring's Args section documents `observed_spreads: Dict mapping issuer ID string to observed spread (float)` and `runtime_tags: Optional dict ...`, and the example calls `decompose_levels(model, {"ISSUER-A": 120.5}, 100.0, "2024-03-29")` — which raises TypeError at runtime since PyO3 will not coerce a dict to str (the example is doctest:+SKIP so this is never caught). The credit module also has no `#[pyo3(text_signature = ...)]` on any constructor or method, contrary to the binding standards checklist; the .pyi stub is correct.

**Impact.** Copy-pasting the documented example fails immediately; misleading API docs on the primary credit decomposition entry point increase support friction, though no numeric impact.

**Fix.** Rename the Args entries to observed_spreads_json / runtime_tags_json, state they are JSON strings, and fix the example to `decompose_levels(model, json.dumps({"ISSUER-A": 120.5}), 100.0, "2024-03-29")`; add text_signature attributes to the public constructors/methods in this module.

```rust
///     >>> snap = decompose_levels(model, {"ISSUER-A": 120.5}, 100.0, "2024-03-29")  # doctest: +SKIP
```

---

### MN19 (Minor) — `pub inner` fields deviate from the wasm code-standard `pub(crate) inner` pattern

**Location:** `finstack-quant-wasm/src/api/factor_model/mod.rs:44`
**Area:** wasm-bindings

**Issue.** `WasmCreditFactorModel.inner` (line 44), `WasmLevelsAtDate.inner` (line 123), and `WasmPeriodDecomposition.inner` (line 146) are declared `pub` with `#[wasm_bindgen(skip)]`, while the project wasm standard mandates named structs with `pub(crate) inner`. The same file already follows the standard inconsistently: `WasmCreditCalibrator.inner` (line 79) and `WasmFactorCovarianceForecast.model` (line 245) are private. `pub` needlessly re-exports the raw core types through finstack-quant-wasm's Rust public API and requires the `skip` attribute that `pub(crate)` would make unnecessary; all intra-crate consumers (decompose_levels at line 201, the forecast constructor at line 254) only need crate visibility.

**Impact.** API-surface leakage and pattern drift in the bindings crate; no runtime effect. Future refactors of the core types become semver-visible through finstack-quant-wasm.

**Fix.** Change the three fields to `pub(crate) inner` and drop the now-redundant `#[wasm_bindgen(skip)]` attributes, matching the wasm code-standards struct pattern used elsewhere in the file.

```rust
#[wasm_bindgen(skip)]
    /// Underlying Rust value (not exposed to JS).
    pub inner: finstack_quant_factor_model::credit::hierarchy::CreditFactorModel,
```

---

### MN20 (Minor) — WASM handles are toJson-only while Python exposes typed accessors; the asymmetry is not recorded in the parity contract

**Location:** `finstack-quant-wasm/src/api/factor_model/mod.rs:127`
**Area:** wasm-bindings

**Issue.** The Python bindings (finstack-quant-py/src/bindings/factor_model/credit.rs) expose typed accessors on every handle — CreditFactorModel.{schema_version, as_of, n_levels, n_issuers, n_factors, level_names, issuer_ids, factor_ids}, LevelsAtDate.{date, generic, n_levels, level_values, adder}, PeriodDecomposition.{from_date, to_date, d_generic, n_levels, level_deltas, d_adder} — all present in the .pyi stub. The WASM wrappers expose only `toJson()` on the corresponding classes (WasmLevelsAtDate impl at lines 126-133 contains a single method). The opaque-handle design is documented in code comments ('The full data is available via toJson'), but parity_contract.toml's factor_model credit note (line 89) lists only the type names and, unlike the `[wasm_core_subset]` block (lines 753-789) which explicitly documents the agreed WASM subset for core, nothing records this accessor asymmetry as intentional — so reviewers and the parity tooling cannot distinguish deliberate subset from drift. Additionally, since LevelsAtDate derives only Serialize (decomposition.rs:53), toJson is strictly one-way: JS callers cannot persist a snapshot and rehydrate it later for decomposePeriod across sessions.

**Impact.** JS callers must JSON.parse full pretty-printed payloads to read a single scalar (extra boundary copies for large issuer universes), and the undocumented asymmetry invites either accidental divergence or unnecessary 'fix the drift' churn in future parity reviews.

**Fix.** Either add the cheap scalar getters (asOf, generic, dGeneric, nLevels, schemaVersion) to the wasm classes for parity, or record the toJson-only WASM subset for factor_model.credit in parity_contract.toml alongside the existing wasm_core_subset documentation pattern.

```rust
impl WasmLevelsAtDate {
    /// Serialize the snapshot to JSON.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
```

---

### MN21 (Minor) — Round-trip serialization tests are self-referential and pin no wire format for the report types

**Location:** `finstack-quant/portfolio/tests/factor_model_serialization.rs:24`
**Area:** consumers-integration

**Issue.** assert_roundtrip_value serializes a value and compares it to its own re-serialization, so it passes under ANY serde field rename or representation change. Only RiskDecomposition has a pinned legacy-JSON fixture (line 100); FactorAssignmentReport, PositionAssignment, UnmatchedEntry, FactorContributionDelta, WhatIfResult, StressResult and MarketDependency/CurveType tag spellings have no literal-JSON anchor, so a schema-breaking rename in these long-lived report payloads would ship undetected despite the workspace's serde-stability invariant.

**Impact.** Silent wire-format drift for factor-model risk reports consumed by downstream pipelines and bindings; breakage would only surface at consumer deserialization time.

**Fix.** Add literal-JSON fixtures (like the pre-PR-6 RiskDecomposition test) asserting exact field names/tags for each exported report type, or golden-file the serialized forms under tests/data/.

```rust
let restored = roundtrip_json(value);
assert_eq!(
    serde_json::to_value(value).expect("value serialization should succeed"),
    serde_json::to_value(&restored).expect("value reserialization should succeed")
);
```

---

### MN22 (Minor) — diagonal_factor_contribution reads the pivoted factor's diagonal as if it were the unpivoted Cholesky L[i,i]

**Location:** `finstack-quant/valuations/src/correlation/factor_model.rs:205`
**Area:** consumers-integration

**Issue.** For the Multi variant the method returns `z * factor_matrix()[i*n+i] * vol[i]` and documents it as "the diagonal Cholesky contribution L[i,i]". But CorrelationFactor comes from pivoted Cholesky and (per core/src/math/linalg.rs:160-162) after unpermutation is "not guaranteed to remain lower triangular", so its diagonal element is not the conditional loading the Two-factor branch's L[1,1]=sqrt(1-rho^2) convention implies. Out-of-range factor_index also silently returns 0.0 instead of erroring. There are currently no production callers (only in-module tests), so this is a latent API footgun rather than an active mispricing.

**Impact.** Any future caller composing per-factor contributions from this method on a pivot-reordered correlation matrix would get inconsistent decompositions that don't reproduce the target correlation; silent 0.0 for bad indices hides indexing bugs.

**Fix.** Either remove the method (no production users) or compute the diagonal of an unpivoted factorization, and return Result/panic on out-of-range factor_index instead of 0.0.

```rust
let l_ii = m.cholesky_factor.factor_matrix()[factor_index * n + factor_index];
z * l_ii * m.volatilities[factor_index]
```

---

### MN23 (Minor) — generate_correlated_factors_into discards the Result of CorrelationFactor::apply

**Location:** `finstack-quant/valuations/src/correlation/factor_model.rs:880`
**Area:** consumers-integration

**Issue.** `let _ = self.cholesky_factor.apply(independent_z, out);` swallows a potential CholeskyError::DimensionMismatch. The two asserts above check lengths against self.num_factors, but nothing ties cholesky_factor.n() to num_factors at this call site; if that internal invariant is ever broken (e.g. a future constructor change), `out` is left untouched (zeros) and then scaled by vols — silent zero correlated shocks in a Monte Carlo path.

**Impact.** Defense-in-depth gap in a simulation hot path: an internal inconsistency would degrade to silently uncorrelated/zero factors instead of an error.

**Fix.** Replace `let _ =` with `.expect(...)` carrying the invariant message, or debug_assert on the returned Result, so the impossible-by-construction case fails loudly.

```rust
let _ = self.cholesky_factor.apply(independent_z, out);
```

---

## Reviewed and rejected (refuted findings)

Two candidate findings were killed by the adversarial verification pass; recorded
here so future reviews do not re-raise them:

- **"Parity test is degenerate; the reconciliation invariant is untested anywhere
  in the workspace"** (`finstack-quant/factor-model/tests/credit_peel_parity.rs:18`,
  originally Major). The narrow facts about `credit_peel_parity.rs` are accurate
  (1 issuer, 1 level, `GloballyOff`, adder unasserted), but the headline is false:
  the reconciliation invariant and multi-issuer/multi-level decomposition behavior
  are exercised end-to-end in `finstack-quant/valuations/tests/credit_decomposition.rs`
  and `finstack-quant/valuations/tests/credit_calibration.rs`, which the original finder
  did not read. The local test's narrowness is folded into MN-class test-hygiene
  notes instead.
- **"`LevelsAtDate`/`PeriodDecomposition` are Serialize-only; persisted snapshots
  cannot be reloaded"** (`finstack-quant/factor-model/src/credit/decomposition.rs:53`,
  originally Moderate). Verifiers found no production workflow that persists and
  reloads snapshots through the Rust API; both bindings intentionally expose
  JSON-out-only result views, and recomputing the `from` snapshot is the
  documented pattern. A `Deserialize` derive remains a cheap ergonomic option but
  is not a defect.

## Open questions / assumptions

1. **Tag-migration semantics for `decompose_period` (MD-class today, could be
   worse).** When an issuer's tags change between snapshots (rating migration —
   a routine monthly event), the documented 1e-10 reconciliation identity silently
   fails for that issuer. Is the intended contract "same model, same tags" — and
   if so, should `decompose_period` detect and report migrated issuers rather than
   silently emitting partial deltas?
2. **Covariance unit contract.** `FactorCovarianceMatrix` documents entries only
   as "typically annual variance/covariance for the factor returns". The credit
   calibrator produces bp²-of-spread-return variances while `BumpSizeConfig`
   sensitivities are per-bp/per-%/per-vol-pt. Confirm the intended convention and
   stamp it (e.g. a units field or doc contract) — the 1e8 bp²-vs-decimal² trap is
   live for hand-built matrices.
3. **`vol_points` downstream interpretation.** The fix for M-class
   `FactorBumpUnit::Absolute` (default 1.0 vs "0.01 = one vol point") needs a
   decision: change the default to 0.01, or redefine `Absolute` as vol-points with
   a /100 conversion in `to_fraction`. Either way the WASM/Python docs must agree.
4. **`asof_spreads` coverage convention.** Is partial coverage ever legitimate
   (sparse panel at month-end)? If yes, the artifact needs an explicit
   per-issuer "anchor missing" marker rather than 0.0; if no, calibration should
   hard-error.
5. **GIL during calibration.** Confirm whether `CreditCalibrator.calibrate` is
   expected to release the GIL (project standard says heavy compute releases it
   inside core; calibration currently holds it end-to-end).

## Brief summary

This is a well-architected crate with unusually strong determinism discipline:
BTreeMap-everywhere, sorted artifacts, byte-stable JSON, a shared
single-observation peel kernel reused by calibration anchoring and runtime
decomposition (preventing the classic drift-between-twins bug), Bessel-corrected
factor variances with a documented rationale, pairwise-overlap correlation with a
regression test pinning the exact prior bug, and PSD repair via nearest-correlation
projection on the strategies that need it. The peel/decomposition math itself
checks out: the reconciliation identity holds per snapshot by construction and is
pinned end-to-end in the valuations test suites.

The residual risk concentrates in three places. First, the silent-defaulting theme
above — none of the Majors is a wrong formula; all are wrong *silences*. Second,
the factor-id namespace is string-fragile: `"."` in a tag value corrupts identity
in three independent places (calibration synth-tags, fold-up parent computation,
matcher aliasing), and the one validator that would catch the resulting mismatch
(`validate_matching_factor_ids`) is wired to nothing. Third, the consumer boundary:
the portfolio delta engine and attribution CS01 paths sum native-currency numbers
raw, which violates the workspace's own currency-safety invariant on multi-currency
books, and the parametric decomposer's `max(0.0, …)` NaN-swallow can print a
zero-risk report from poisoned sensitivities.

Nothing here is a wrong-price Blocker on clean inputs; with complete, well-formed
data the engine produces defensible numbers. The fixes are mostly cheap
validation/wiring (call the existing validator, reject dots in tags, error on
unsorted betas, length-check beta vectors, propagate NaN, add currency checks) and
would move the crate from "correct on clean data" to "trustworthy under production
data quality".

## Quant notes

- The sequential peel is a hierarchical variance decomposition, not an orthogonal
  factor rotation: bucket means are estimated on residuals level-by-level, so
  level ordering (broadest→narrowest) is part of the model definition. That is a
  legitimate, well-precedented design (cf. BARRA-style nested industry/issuer
  models) but worth stating in docs: re-ordering `hierarchy.levels` changes the
  decomposition, not just labels.
- Self-inclusion of an issuer in its own bucket mean (MD) is the same
  small-bucket bias the fold-up thresholds exist to police; leave-one-out or a
  shrinkage on bucket means for counts near threshold would tighten it without
  changing the architecture.
- `BetaShrinkage::TowardOne` is James–Stein-flavored but with a fixed α; if the
  desk later wants data-driven shrinkage, Vasicek (1973) precision-weighted
  shrinkage drops in cleanly at the same call site.
- The Ridge strategy's documented Σ = D·ρ·D + αI deliberately decouples the
  stored `static_correlation` from the implied correlation of `config.covariance`
  (off-diagonals shrink by σᵢσⱼ/√((σᵢ²+α)(σⱼ²+α))); MD-class finding asks for the
  consumer-facing doc to say which matrix is authoritative for which use.
- Population (÷n) vs sample (÷n−1) variance is mixed between adder vols and
  factor vols; harmless at n≥24 but the n≥24 precondition is not enforced, and
  the inconsistency itself is the kind of thing a model-validation team flags.

## Coverage and residual risk (what this review did NOT cover)

From the completeness critic, confirmed by spot-checks:

1. `finstack-quant-py/src/bindings/portfolio/sensitivity.rs` and
   `finstack-quant-wasm/src/api/portfolio/sensitivity.rs` (user-JSON →
   `ParametricDecomposer` paths) were outside every slice; the WASM non-finite
   finding class likely applies there too.
2. `finstack-quant/portfolio/src/factor_model/` engine internals reachable from the
   reviewed bindings: `position_risk.rs` (ES truncated-normal moments,
   historical tail quantiles), `whatif.rs`, `simulation.rs`,
   `credit_vol_forecast.rs`, `math.rs` (Beasley–Springer–Moro probit constants —
   unverified), ~60% of `model.rs`. **A dedicated portfolio-factor-risk-engine
   review is the natural follow-up.**
3. `finstack-quant/attribution/src/credit_decomposition.rs`, most of
   `credit_cascade.rs`, and the ~1.9k lines of attribution integration tests.
4. `finstack-quant/valuations/tests/credit_calibration.rs` (1,740 ln) and
   `credit_decomposition.rs` (652 ln) were used to refute one finding but not
   themselves line-reviewed.
5. `finstack-quant/analytics/src/correlation/nearest_correlation.rs` (Higham repair) —
   load-bearing for `FullSampleRepaired`/`Ridge`, unreviewed.
6. No property tests exist for peel sum-to-total, anchor identity, or
   period additivity; no golden calibration fixture pins exact betas/vols bytes.
7. No Node-side facade test executes the `factor_model` WASM namespace.
8. The `credit_factor_hierarchy.ipynb` example notebook was not checked for
   stale API usage.
