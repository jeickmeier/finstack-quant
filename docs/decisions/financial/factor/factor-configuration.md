# FACTOR-001: Factor definitions, units, and configuration validation

**Decision status:** Accepted 2026-11-17 for ordered factor bases, typed shock coordinates and checked configuration.

**Implementation status:** Ordered factor IDs, matcher-universe checks and risk-confidence validation are implemented. Explicit bump coordinates, full configuration admission and checked risk-basis alignment remain pending.

**Validation status:** All 343 Factor Rust tests passed on 2026-11-17 against current behavior. The accepted configuration rules and host and downstream consumers still need qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Bind factor definitions, sensitivities and covariance to the same ordered factor basis. Require unique, exact, nonblank factor IDs, matching axes and coordinates, a declared covariance horizon and one reporting currency for monetary sensitivities. Reject mismatches before computing risk. [Matrix invariants](matrix-invariants.md) · [Factor risk](factor-risk.md).
2. Require an explicit positive finite magnitude in the declared unit for every active finite-difference bump. Retire implicit one-unit defaults and the custom-factor rate fallback. Distinguish relative spot and FX moves from additive volatility moves. For example, one absolute volatility point moves sigma from 20% to 21%; a 1% relative shift moves it to 20.2%. Keep assignment-driven credit distinct from market-mapped shocks. [Portfolio shock and routing contract](../portfolio-execution/12-factor-shocks-and-routing.md#accepted-decision-for-checked-portfolio-factor-shocks-and-routing).
3. Share Rust admission checks at construction and restoration. Reject incompatible factor and mapping combinations, duplicate targets, unknown bump IDs, and invalid tenor grids or weights. Validate matcher references against the declared universe. Require an explicit risk measure and finite VaR or ES confidence strictly in `(0.5, 1)`. Keep Strict as the default. Warn can return an incomplete assignment report under the [accepted coverage rules](../portfolio-attribution/12-factor-assignment-and-credit-exposures.md#accepted-decision-for-checked-factor-assignment-and-credit-exposures).

## Observed choice and rationale

Represent the factor universe as ordered typed definitions, an aligned covariance matrix, declarative matching, and a selected risk measure. The observed rationale is a single configuration consumed by portfolio sensitivity engines; the numerical bump execution belongs to that downstream scope.

## Input domain and conventions

A definition associates a factor ID/type with parallel curves, tenor-weighted curve buckets, equity tickers, an FX pair, or volatility surfaces. Rate/credit/inflation/custom bump defaults are in bp; equity/commodity/FX defaults are percent; volatility defaults are vol points (1 = 0.01 absolute vol). Custom types inherit the rates default. Covariance must be expressed in the same factor coordinates and horizon as sensitivities. This crate does not attach a reporting currency or automatically reconcile covariance units.

## Algorithm stages

1. Resolve a per-factor bump override before the type default; every default magnitude is 1.
2. Validate exact ordered equality of definition IDs and covariance IDs. Covariance construction separately rejects duplicate IDs.
3. Validate confidence for VaR/ES: finite and strictly between 0.5 and 1. Variance is the default measure.
4. Enumerate every declaratively emitted factor ID and reject an undeclared ID; reject duplicate credit issuer rows both inside one matcher and across cascade members.
5. A strict envelope loader decodes the exact `finstack_quant.factor_model_config/1` marker, then invokes configuration validation; ordinary configuration deserialization does not run the whole validation.

## Selection and controls

`unmatched_policy=None` selects Strict at the portfolio boundary; Warn is explicit. `bump_config=None` selects type defaults. Volatility uses the shared `Percent` bump tag even though its configured magnitude is absolute vol points; consumers must preserve the described distinction. Inflation and base-correlation dependencies are not automatically assigned to Discount/Forward/Credit: `matches_dependency_type` has explicit branches. A credit index matches Credit but not the Hazard curve-type filter; a credit curve matches both.

## Limitations, failures, and fallbacks

Configuration validation does not validate bump magnitudes, unknown override IDs, compatibility between factor type and mapping, or tenor-weight shapes. Raw in-memory configs and `FactorModelConfigEnvelope::new` can therefore carry values beyond this validator. This is a validation-scope fact, not proof that every downstream engine accepts them.

## Alternatives and tradeoffs

Typed mappings carry the unit of each financial operation. Shared admission lets pricing and risk consumers use the same configuration contract while retaining operation-specific checks on actual market targets and delivered moves.

## Implementation and evidence

- [src/config.rs](../../../../finstack-quant/factor/src/config.rs) — inspected implementation.
- [src/primitives/definition.rs](../../../../finstack-quant/factor/src/primitives/definition.rs) — inspected implementation.
- [src/primitives/factor_types.rs](../../../../finstack-quant/factor/src/primitives/factor_types.rs) — inspected implementation.
- [src/primitives/dependency.rs](../../../../finstack-quant/factor/src/primitives/dependency.rs) — inspected implementation.
- [src/envelope.rs](../../../../finstack-quant/factor/src/envelope.rs) — inspected implementation.

[tests/multi_asset_config.rs](../../../../finstack-quant/factor/tests/multi_asset_config.rs) covers multi-asset, exact-axis and strict-envelope behavior. [src/config.rs](../../../../finstack-quant/factor/src/config.rs) contains matcher-universe, confidence and volatility unit tests. These tests passed against the current implementation. Qualify the accepted admission rules, host parity and downstream unit consistency.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
