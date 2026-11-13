# MODELS-AND-VALUATIONS-CREDIT-06: Recovery dilution, LGD, EAD and priority allocation

- Decision status: **Accepted — full package on 2026-11-12**, including the scoped CORE-NUMERICS-12 amendment.
- Implementation status: Current behavior observed in the source below. The accepted contract requires Rust and caller reconciliation.
- Validation status: Independent scalar and allocation examples evaluated. Production, host, empirical and regulatory qualification were not run.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked exposure, loss and recovery allocations

1. **Use explicitly selected models and a common amount basis.** Exposure,
   dilution, workout, downturn, beta recovery and the priority waterfall
   are separately invoked. Binary64 amounts are scenario values on one
   caller-supplied currency, monetary scale and financial basis. The caller
   supplies valuation or observation dates where required by that basis.
   Validate inputs before zero identities. Check required finite results,
   range and loss of positive amounts. Actual settlement or posting uses
   the accepted money and rounding contract. [Core money](../core-conventions/012-money-decimal-precision.md).

2. **Retain explicit recovery dilution rules.** Require base recovery
   `R0` in `[0,1]` and finite nonnegative current notional `N`. The
   dilution variants require finite positive reference notional `N0`.
   At zero notional return zero. For positive notional through `N0`,
   return `R0`. A constant uses `R0` at all positive notionals and stores
   no reference notional. Above `N0`, the inverse rule is `R0*N0/N`, the
   inverse-power rule is `R0*(N0/N)^a` for `0<a<=1`, and the floored
   inverse rule is `max(f,R0*N0/N)`. Floored linear decline uses
   `max(f,R0*(1−s*(N/N0−1)))` for finite `s>=0`. Each declared floor
   satisfies `0<=f<=R0`. Evaluate ratios and products in scale, including
   deciding whether an explicit floor binds without overflowing a ratio.
   Fail if a required positive rate is lost. Keep the named floor and
   base plateau as model rules and remove the blanket output clamp.
   These rules model recovery as a fraction of the selected claim. A
   declining fraction can coexist with increasing cash recovery. The
   bond owns flat versus dynamic selection, accreted notional at the
   applicable default event, and settlement. [Bond structural simulation](../valuations-bonds/08-merton-simulation.md).

3. **Require explicit CCF and distinguish observed LEQ.** For finite
   nonnegative drawn `D` and undrawn `U`, require a caller-supplied
   projected CCF in `[0,1]` and compute `EAD=D+CCF*U` with checked
   arithmetic. A fully drawn exposure has `U=0`. A full-draw projection
   explicitly supplies `CCF=1`. Remove the generic 75% revolver preset.
   Check commitment `D+U` separately. Compute utilization `D/(D+U)`
   with scaling even when the commitment amount itself is out of range.
   Zero commitment has unavailable utilization. Zero EAD is valid.
   For finite nonnegative observed EAD on the stated default measurement
   basis, compute raw LEQ `(EAD_observed−D)/U` for `U>0`, retaining
   negative and above-one observations. `U=0` gives unavailable LEQ.
   The caller owns the reference drawn and undrawn basis, the observation
   window, and any subsequent estimation of CCF. [OCC study of observed loan equivalency](https://www.occ.gov/publications-and-resources/publications/economics/working-papers-archived/pub-econ-working-paper-2009-2.pdf),
   [ECL exposure policy](../statements-analytics-credit/12-ecl-exposure-and-pd.md).

4. **Keep one bounded, co-timed workout scenario.** Require finite
   positive EAD, explicitly supplied finite nonnegative resolution
   duration `T`, finite nonnegative annual effective discount rate `r`
   and explicit finite nonnegative direct and indirect EAD cost rates
   `c_d,c_i`. Admit an explicitly empty collateral list. Each eligible
   piece has a unique ID, finite nonnegative pre-haircut value `V_i` and
   explicit haircut `h_i` in `[0,1]`. Asset class describes the input.
   Gross proceeds are `G=min(EAD,Σ V_i*(1−h_i))`; accumulate up to the
   cap after validating all pieces. Both receipts and costs occur at
   resolution in this profile. Compute signed present net proceeds
   `W=(G−EAD*(c_d+c_i))*exp(−T*log1p(r))` with checked scaling. Expose
   `W`, bounded net recovery `max(W,0)`, recovery rate from bounded net
   recovery divided directly by EAD, and `LGD=1−recovery_rate`. Preserve
   a representable small recovery rate even when LGD rounds to one.
   Remove the extra LGD clamp and embedded duration, rate and cost
   defaults. Claimant-available proceeds are caller-prepared. A pledge,
   haircut, expense or estate distribution enters only once. Dated
   receipts, expenses and post-default draw measurement belong to a
   separate economic-loss policy. [Basel Committee guidance on LGD measurement](https://www.bis.org/publications/200507-guidelines-guidance-estimation-loss-given-default-paragraph-468-framework-document).

5. **Keep downturn adjustment as an explicit stress heuristic.** For
   base LGD `L` and correlation `ρ` in `[0,1]`, finite sensitivity `s>=0`
   and normal quantile probability `q` in `[.5,1)`, use
   `min(1,L+s*sqrt(ρ)*Φ⁻¹(q)*sqrt(L*(1−L)))`. The median, zero
   sensitivity, zero correlation and zero dispersion leave the base
   unchanged. Treat the cap as part of the formula and evaluate it
   without overflowing an unneeded intermediate. Rename the regulatory
   floor variant to `AddOnAndFloor` with `min(1,max(L+add_on,floor))`
   and explicit `add_on,floor` in `[0,1]`. Fail unresolved calculations.
   Remove automatic and regulatory-labelled presets. Scenario parameters
   and any current jurisdiction-specific qualification belong to their
   owners. [Basel downturn LGD background](https://www.bis.org/publications/200507-guidelines-guidance-estimation-loss-given-default-paragraph-468-framework-document).

6. **Derive checked beta recovery shapes from explicit moments.** Require
   finite `0<m<1`, positive finite deviation `s` and representable
   `0<v=s²<m(1−m)`. Derive `α=m*(m(1−m)−v)/v` and
   `β=(1−m)*(m(1−m)−v)/v` in scale. Require both shapes finite and
   positive, without requiring their sum to be finite. Serialize only
   the input moments. Rebuild and validate the derived shapes on restore.
   The interior mode is available only when both shapes exceed one and
   uses the checked ratio of `α−1` to `(α−1)+(β−1)`. Mean LGD is `1−m`.
   Reuse the accepted checked beta sampler. The seeded batch requires
   explicit `u64` PCG seed and stream, checked count and allocation,
   fixed draw order and atomic failure. Zero requested draws return an
   empty batch. [Beta sampling](../core-numerics/beta-gamma-sampling.md),
   [PCG profile](../core-numerics/pseudorandom-and-bridge.md).

7. **Check the beta inverse on the actual returned value.** Require
   finite `p` in `[0,1]` with exact endpoints. For interior `p`, reuse
   the bounded incomplete-beta and inverse-beta routines, require
   `0<x<1`, and check the smaller supplied tail on the returned `x`.
   Compare `log F(x)` with `log(p)` for `p<=.5`. For `p>.5` compare
   the directly evaluated log survival with `log1p(−p)`. Require an
   absolute log residual at most `1e−8`.
   Reject a failed gate, unresolved arithmetic or exhaustion. No
   all-shape quantile accuracy claim follows from the numerical gate.
   This includes a scoped amendment to CORE-NUMERICS-12's API boundary
   for one Rust beta-quantile entry used by `BetaRecovery`. The
   incomplete-beta routines remain internal, and host exposure remains
   on the existing recovery model. [Accepted core amendment](../core-numerics/distribution-functions.md#accepted-amendment-checked-rust-beta-quantile),
   [NIST beta identities](https://dlmf.nist.gov/8.17).

8. **Require explicitly selected frozen recovery assumptions.** Admit
   a versioned snapshot with an exact ID, qualified source or declared
   scenario provenance, observation window when applicable, recovery
   basis, population and class mapping. Validate unique canonical
   seniority classes and each row's moments. A snapshot can cover a
   subset. An absent requested class or ID fails. Remove default-first
   listing, implicit seniority selection and the existing approximate
   agency-labelled recovery IDs from financial lookup. Existing numbers
   used in examples may be explicitly labelled illustrative assumptions.
   Remove implicit workout and downturn selections independently of
   credit rating-factor and PD-scale data. No rating or waterfall label
   implies a beta recovery class. [PD and rating-data decisions](02-pd-scales-and-rating-data.md),
   [empirical default and recovery research](https://www.jstor.org/stable/10.1086/497044).

9. **Require one net inclusive estate and exclusive collateral pools.**
   Supply finite nonnegative distributable estate `E` on one net,
   post-haircut basis, including the full net value of each pledged
   pool exactly once. Supply unique trimmed case-sensitive claim IDs,
   finite nonnegative principal, accrued and allowed fees, and an
   explicit numeric deficiency priority. Labels describe the claims.
   Each optional collateral pool has a unique ID, finite nonnegative
   gross value and haircut in `[0,1]`. The caller partitions and
   assigns pools exclusively. Reject duplicate pools and reject a
   checked sum of full net pool values above `E` before applying claim
   caps. Pay secured recovery `min(net_pool,allowed_claim)` and deduct
   actual secured payments from `E`. Surplus pledged value remains in
   the general estate. The caller owns claim allowance, lien sharing,
   collateral valuation, any costs already netted and entitlement to
   share in the general estate. [UNCITRAL insolvency guidance](https://uncitral.un.org/sites/default/files/media-documents/uncitral/en/05-80722_ebook.pdf).

10. **Distribute general estate by checked deficiency pro rata.** Visit
    ascending supplied ranks. Pay each fully fundable rank's deficiencies
    in full. In the first underfunded rank with budget `A`, allocate
    `A*d_i/Σd` for each remaining positive deficiency `d_i`. No junior
    rank receives general funds after that rank. Use checked scaled
    ratios and compensated totals. Keep presentation order by rank and
    original input order, with no special payment for the last claim.
    Require nonnegative payouts within allowed claims, class budgets and
    total estate. Reject unresolved arithmetic, overspend and required
    positive shares lost to zero. Use an output-only maximum relative
    discrepancy of `64*EPSILON` on the affected amount scale, with exact
    zero requiring exact zero. Report bounded numerical underfill as
    undistributed value and identify it separately. Keep it out of
    junior allocations. Report pledged and general recovery, total,
    deficiency, recovery rate and undistributed estate. A zero claim
    has zero payment and unavailable rate. An empty claim list leaves
    the entire estate undistributed. Remove the always-true
    `apr_satisfied` field. The ordering implements supplied ranks and
    exclusive pool terms, while legal priority and plan compliance stay
    with their owners.

11. **Restore validated state and reconcile actual consumers.** Use
    private immutable model inputs, strict variants and the same
    validation on direct construction and restore. Keep outputs with
    computed allocation and provenance claims as result views. Rust
    owns formulas, units, arithmetic and failure semantics. Reconcile
    Python, WASM, generated schemas and declarations, parity, pricing
    overrides, and the UI workout and waterfall routes and fixtures.
    Propagate checked recovery through both Merton bond default branches
    and checked EAD/downturn adjustments through statements ECL. The
    bond owns event timing and settlement. ECL owns its measurement,
    scenario selection and exposure policy. The workout calculator,
    beta moments and waterfall do not silently substitute for each
    other or for the separate portfolio recovery dependence model.
    [Portfolio recovery dependence](10-portfolio-loss-and-correlated-recovery.md).

## Observed behavior

- Dilution uses an unscaled notional ratio, returns zero for nonpositive
  notional, and clamps outputs. EAD offers an implicit `.75` revolver
  factor. Its amount and ratio getters are unchecked, and utilization
  at zero commitment is reported as zero.
- The workout builder loads a default duration, discount rate and cost
  profile, even when all fields are supplied. It derives the recovery
  rate by complementing LGD after net recovery was already divided by
  EAD. Downturn accepts probabilities below the median and uses an
  embedded regulatory-labelled add-on and floor. Seniority lookup lists
  the default first. The embedded source labels describe approximate
  moments, and the runtime conversion omits the study window. A small
  deviation can underflow variance and admit infinite beta shapes.
  Seeded recovery draws use only a seed, and quantiles delegate directly
  to the statrs inverse.
- The waterfall compares estate with the sum of claim-capped pledged
  payments. It uses the last row to absorb pro rata residue, can reduce
  prior computed payouts in reverse order, and always emits
  `apr_satisfied=true`. The UI renders that flag. [Embedded assumptions](../../../../finstack-quant/models/data/credit/credit_assumptions.v1.json),
  [model UI worker](../../../../finstack-quant-ui/registry/shared/file/services/models.ts).

## Evidence

Owners: [dynamic_recovery.rs](../../../../finstack-quant/models/src/credit/dynamic_recovery.rs), [LGD implementations](../../../../finstack-quant/models/src/credit/lgd), [recovery_waterfall.rs](../../../../finstack-quant/models/src/credit/recovery_waterfall.rs), [registry.rs](../../../../finstack-quant/models/src/credit/registry.rs). [recovery_waterfall.rs tests](../../../../finstack-quant/models/tests/recovery_waterfall.rs) check payout conservation; inline LGD tests check domains, beta moments and formulas; none establishes empirical default-loss calibration. References: [LGD](../../../REFERENCES.md#schuermann-2004-lgd), [Recovery dependence](../../../REFERENCES.md#altman-et-al-2005-recovery).

### Independent review evidence

- With `R0=.4,N0=1e−300,N=1e300,a=.5`, the inverse-power recovery is
  about `4e−301`. The raw ratio underflows. For a floored linear curve
  with `R0=.4,N0=100,s=.5,f=.1`, moving from `N=100` to `120` reduces
  the fraction from `.4` to `.36` and increases cash from `40` to `43.2`.
  Drawn `60` and undrawn `40` yield LEQ `−.5` at observed EAD `40`
  and `1.5` at `120`. At equal maximum binary64 balances, utilization
  remains `.5` even though their total commitment overflows.
- At EAD `100`, gross proceeds `50`, costs of `5%` and `3%`, annual
  effective rate `.10` and resolution in two years, bounded LGD is
  `0.6528925620` with co-timed costs. The same costs at default yield
  `0.6667768595`. With no gross proceeds, raw loss exceeds EAD and the
  co-timed bounded model gives LGD `1`. A positive recovery fraction of
  `1e−22` survives direct division even when LGD rounds to one.
- In the existing stress formula `L=.45,ρ=.15,s=.4,q=.01` gives
  `0.2707051230`. The proposed downturn domain rejects that probability.
  With `m=.5,s=3e−155`, both beta shapes are about `1.3889e308` even
  though their sum overflows. With `m=.5,s=1e−200`, represented
  variance vanishes. For `Beta(2,1)` at the largest binary64 `p<1`,
  the nearest interior quantile has a log-tail residual about `log 2`,
  which the proposed inverse gate rejects.
- Estate `100` with a single full net pledged pool of `120` is
  inconsistent even if its secured claim is only `20`. At estate
  `130` that claim receives `20` secured and leaves `110` for general
  distribution. For an estate of `100`, two same-rank `100` claims,
  `45` exclusively pledged to the first and a junior `50` claim,
  exact recoveries are `2000/31`, `1100/31`, and zero. Three equal
  deficiencies sharing one unit have equal rounded thirds with
  compensated total one. In the scalar reproduction, the current
  last-row rule gives that row two ulps more than the other shares.
- Independent formula checks and source test inspection cover these
  particular cases. Production qualification still requires range,
  inverse-tail, sampling, conservation, restore, host, product and
  source-applicability evidence.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
