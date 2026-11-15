# CALIBRATION-FRAMEWORK-017: Forward-moneyness calendar and call-spread checks

- Decision status: **Accepted 2026-11-15 — complete sampled Black surface validation and publication package**.
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent overlap, price-anchor and between-node checks verified.** Production diagnostic, publication, restoration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for surface validation and publication

1. **Use the accepted Black surface diagnostic.** Require an unshifted
   Black surface with strictly positive expiry pillars and absolute
   strikes, finite nonnegative annual sigma, and one finite positive
   forward bound to each exact expiry. Verify underlying, strike units,
   valuation cut and the source clock where applicable. Zero sigma uses
   intrinsic. Actual zero-time pricing belongs to the product lifecycle.
   Normal, shifted Black, delta and tenor grids and cubes require their
   own diagnostic profiles.
   [Volatility source admission](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) ·
   [Accepted diagnostic](../models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage) ·
   [Checked Black unit price](../models-analytic/forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles).

2. **Require qualified carry for calendar tests.** Compare
   `w = T σ²` at fixed `K/F` on the same underlying under deterministic
   discounting and proportional carry. The ordering then agrees with
   that of normalized calls `C_undisc/F` under the common pricing
   measure. An absolute cash-dividend schedule makes this calendar
   test inapplicable. The same-expiry tests retain their own admission.
   Use a separately elected finite nonnegative absolute total-variance
   tolerance. Compare the checked signed difference to its negative.
   [Gatheral and Jacquier, Lemma 2.1](https://arxiv.org/pdf/1204.0646).

3. **Check the exact common support.** For each adjacent expiry pair
   take the closed intersection of the two `K/F` ranges. Probe the
   exact union of both rows' normalized knots in that intersection
   including its endpoints. Map each probe using each row's actual
   forward and the published strike interpolator. Preserve original
   knot identities and refuse unresolved mappings. Report disjoint
   support as uncovered and singleton overlap as point-only coverage.
   The diagnostic scope ends at the overlap boundaries.
   [Accepted surface lookup](../models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).

4. **Check the undiscounted Black call grid and its anchor.** Check
   `max(F−K,0) ≤ C ≤ F` at each observed strike. Include `(K,C)=(0,F)`
   in the spread tests without a volatility lookup. Check each adjacent
   price difference against `[0,K_next−K]` and each middle price
   against the chord weighted by its actual strike spacing. Compare
   the represented premiums and strikes with the accepted exact
   arithmetic. Use a separately elected finite nonnegative absolute
   tolerance in native unit-premium terms. A single observed strike
   supplies bounds and an anchor vertical with incomplete convexity
   coverage.
   [Price-grid arithmetic](../models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair).

5. **Elect the publication policy.** Require source validity and pricing
   support. Elect sampled checks and their required or advisory
   disposition. An elected volatility cap is explicit in annual sigma
   units. Retire the blanket positive-volatility rule, shared tolerance,
   hidden premium-tolerance floor and default cap. Invalid inputs,
   unresolved arithmetic and failed evaluations produce typed errors.
   Missing required coverage blocks publication. Advisory gaps remain
   incomplete. Report exact scope, checked counts, gaps, native breach
   amounts and limits. Passing sampled constraints covers those probes.
   Between-node behavior, wings and continuous density require separate
   evidence.
   [Accepted curve gate and policy](16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

6. **Validate each delivered grid through the same Rust gate.** Use
   the accepted diagnostic kernel for equity SABR and SVI in direct,
   serial and parallel calibration. Apply the same policy whenever an
   existing supported workflow rematerializes the surface. Report quote
   fit, delivered-grid findings and any elected fitted-SVI density scan
   separately. Retire the duplicate standalone calibration checks in
   favor of the canonical model diagnostic and preserve shared Rust
   outcomes in Python and WASM. Findings apply to the exact artifact.
   Surface repair requires its own explicit operation.
   [Accepted fit report](14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics) ·
   [Accepted equity materialization](../calibration-targets/10-total-variance-materialization.md#accepted-decision-for-explicit-total-variance-materialization-and-delivered-surface-fit) ·
   [Accepted fitted-SVI density scan](../models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance).

## Independent calculation checks

- At expiries `1,2`, forwards `100,200`, strikes `100,150,220`
  and linear-volatility rows `(.30,.10,.10)` and `(.20,.20,.20)`,
  the later-only probe at `K/F=1.1` sees `.0676 < .08`. The omitted
  overlap boundary at `K/F=1` shows `.09 > .08`.
- With `F=100`, calls `30,10` at `K=80,100` pass the observed
  vertical check. Including `(0,100)` gives chord 28 at strike 80
  and convexity excess 2.
- At `F=100,T=1`, strikes `90,110,130` with sigma `.01,.32,.44`
  pass the sampled constraints including the anchor. Linear sigma
  interpolation gives a call of about 6.575088 at strike 100, below
  the call of about 8.929637 at strike 110. The off-grid vertical
  breaches monotonicity.

## Observed choice and rationale at the audit revision

Surface checks use the financial forward associated with each expiry. They compare total variance at fixed forward moneyness and validate undiscounted call prices, rather than comparing volatility at fixed absolute strike. These choices are coherent diagnostics for the stated positive lognormal domain.

## Observed inputs and stages

The checker requires one positive finite forward per stored expiry and positive finite strikes, even if arbitrage checks are disabled. The source constructor governs expiry admission. The current regression explicitly checks rejection of a zero-expiry pillar and preservation of small positive expiries.

For adjacent expiries t1,t2, take each later-row strike K2 and compute `k=ln(K2/F2)` and `K1=F1 exp(k)`. Skip the pair if mapped K1 lies outside the common surface strike bounds. Evaluate the surface using the volatility resolver, require finite values and `w=σ²t`, and flag `w2 < w1−tolerance`.

At each expiry, evaluate undiscounted Black calls at all stored strikes. Set price tolerance `max(tolerance,1e-12) × forward`. Every adjacent vertical spread must lie between negative tolerance and strike width plus tolerance. For three strikes, require the middle call at or below the linear chord between outer calls plus tolerance; the interpolation weight accounts for unequal strike spacing. Fewer than two strikes skip call-spread checks; exactly two still check vertical monotonicity/upper bound. Always test every stored vol is positive, finite and≤configured max, even with arbitrage checks disabled. Gather errors; calendar/call-spread summaries include counts and at most five details, while volatility bound messages are collected directly.

## Observed limitations and alternatives

This checks the published sampled surface, including interpolated K1 values, but not every strike/expiry or wings. No-overlap calendar pairs are skipped without an explicit untested-region summary. It assumes positive forwards/strikes and Black calls; normal or shifted surfaces need a different validator. A finite set of convex call nodes does not certify all interpolated values or densities.

Alternatives are analytic parameter restrictions (for example SVI density), constrained fitting, or adaptive call-grid checks including wings and extrapolation. No vendor or theorem-based qualification was executed here.

## Source and validation evidence

- Implementation: [validation/surfaces.rs](../../../../finstack-quant/calibration/src/validation/surfaces.rs) — fixed-moneyness probes, vertical/butterfly inequalities, bounds and finite checks.
- Implementation: [validation/config.rs](../../../../finstack-quant/calibration/src/validation/config.rs) — tolerance/max-vol policy.
- Test evidence: [tests/calibration/validation.rs](../../../../finstack-quant/calibration/tests/calibration/validation.rs) — explicit call-spread violations, fixed-forward-moneyness example, overflow rejection and strict expiry admission. Tests were inspected and remain unrun.
- Reference context: [Gatheral–Jacquier 2014](../../../REFERENCES.md#gatheral-jacquier-2014-svi) — no-arbitrage surface context; not proof these finite probes suffice; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
