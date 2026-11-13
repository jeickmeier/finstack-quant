# MODELS-AND-VALUATIONS-CREDIT-01: Financial distress scores and classification

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing Rust scores, Python/WASM bindings and the credit scorecard observed. Formula, output and caller reconciliation are pending.
- Validation status: Primary references, selected source tests and independent coefficient, scaling and range examples reviewed. Runtime, host and empirical qualification are pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked financial distress scores

1. **Require an explicit model and prepared ratios.** Select
   one of the four Altman forms, Ohlson Model 1, or the named
   Zmijewski probit specification. Require all fields of the
   selected form as finite decimal ratios or its declared
   size/indicator inputs. Admit signed accounting ratios and
   literal zero. The caller owns entity eligibility, statement
   periods, point-in-time availability, denominator definition
   and ratio preparation. An undefined ratio is unavailable at
   that boundary. No substitution, winsorization, normalization
   of raw statements, inferred sector or model fallback.

2. **Use the original public-manufacturer Z coefficients.**
   With `X1=working capital/total assets`,
   `X2=retained earnings/total assets`,
   `X3=EBIT/total assets`, `X4=market equity/book total debt`
   and `X5=sales/total assets`, use
   `Z=1.2*X1+1.4*X2+3.3*X3+0.6*X4+0.999*X5`.
   Equity includes common and preferred shares on the
   qualified market basis. The book denominator includes
   current and long-term debt/liabilities on the source basis.
   The first four coefficients reflect decimal ratio inputs.
   Retain the 1968 sales coefficient `0.999`.
   [Original Altman](../../../REFERENCES.md#altman-1968).

3. **Keep each revised Altman form explicit.** Use
   book equity over book total liabilities as `X4b` in the
   revised profiles. The private-manufacturer `Z'` uses
   `0.717*X1+0.847*X2+3.107*X3+0.420*X4b+0.998*X5`.
   The non-manufacturer `Z''` uses
   `6.56*X1+3.26*X2+6.72*X3+1.05*X4b`.
   The emerging-market corporate score is `3.25+Z''`.
   It exposes the quantitative score. Retire the interpretation
   of the published bond-rating averages as rating cutoffs.
   [Altman revisions](../../../REFERENCES.md#altman-1968).

4. **Retain the Altman bands as descriptive screens.** For
   `Z`, `Z'`, `Z''` and EM, use lower/upper boundaries
   `(1.81,2.99)`, `(1.23,2.90)`, `(1.10,2.60)` and
   `(4.35,5.85)`. Compare the represented checked score
   strictly below or above the respective boundaries for
   Distress or Safe. The closed intervening interval is Grey.
   The last two bands are declared project screens, with
   the EM pair obtained by shifting the Z″ boundaries.
   Zones are screening labels and carry no calibrated
   probability, agency grade or default determination.

5. **Select Ohlson's one-year Model 1.** The original
   study uses U.S. industrial firms from 1970–1976. Use
   its Table 4 specification
   `O=-1.32-0.407*SIZE+6.03*TLTA-1.43*WCTA+0.0757*CLCA-1.72*OENEG-2.37*NITA-1.83*FUTL+0.285*INTWO-0.521*CHIN`.
   `TLTA,WCTA,NITA` use total liabilities, working capital
   and net income over total assets. `CLCA` is current
   liabilities/current assets. `FUTL` uses source-defined
   funds provided by operations over total liabilities.
   `OENEG` is one when liabilities exceed assets. `INTWO`
   is one when both annual net incomes are negative.
   Require each indicator as exact numeric zero or one,
   based on the qualified underlying amounts.
   `CHIN=(NI_t-NI_(t-1))/(abs(NI_t)+abs(NI_(t-1)))` is in
   `[-1,1]`. A zero denominator is unavailable.
   [Ohlson](../../../REFERENCES.md#ohlson-1980).

6. **Require the historical SIZE basis for that score.**
   Accept the caller-prepared natural log of nominal
   total assets reported in USD dollars divided by the
   U.S. GNP price-level index for the year preceding the
   balance-sheet year, on the 1968=100 basis. Require
   positive assets and index at preparation. No implicit
   choice of index, vintage, currency unit or intercept
   adjustment. A common unit change shifts score and
   fitted probability levels even when ranking is preserved.
   Use the stable logistic link for the native result.
   Ohlson's 0.038 was a sample cutoff minimizing the sum
   of class error rates. Retire the automatic 0.019/0.038
   zones.
   [Ohlson size and outcome](../../../REFERENCES.md#ohlson-1980).

7. **Select the named Zmijewski three-factor probit.** Use
   `Y=-4.336-4.513*ROA+5.679*FINL+0.004*LIQ` and
   `Phi(Y)`. `ROA` is net income/total assets, `FINL`
   is source-defined total debt/total assets, and `LIQ`
   is current assets/current liabilities. Require the
   debt-basis input explicitly and retire the ambiguous
   total-liabilities field. Prepare debt consistently
   with the selected historical basis, without a
   generic accounting substitution. Retain the
   positive liquidity coefficient. Use the specified
   vector reported in Shumway's Table 4 with the
   variable definitions in Zmijewski's paper. Retire the
   automatic 0.10/0.50 zones.
   [Zmijewski and coefficient transcription](../../../REFERENCES.md#zmijewski-1984).

8. **Check complete scores before applying links or zones.**
   Use the listed decimal coefficients as the calculation
   target with the represented inputs. Use checked
   scale-aware products and sums, preserving resolvable
   cancellation across large terms. Reject an unresolved
   or unrepresentable complete score as a computation
   failure before classification or transformation.
   Use a sign-stable logistic and the accepted checked
   normal CDF. A finite extreme score can produce a
   binary64 endpoint probability by rounding. Interpret
   endpoints as rounded values of the historical model link.
   [Accepted distribution functions](../core-numerics/distribution-functions.md).

9. **Return a checked model-tagged result and retire automatic
   grading.** Return the finite score and a closed model
   identity. Expose the Altman zone only on its forms and
   the native model probability only on Ohlson and Zmijewski.
   Rename the latter field from implied PD to model
   probability. On restoration validate the model, score,
   field availability and derived results through Rust.
   Remove the default master-scale lookup, nested grade
   report and automatic routing of score probabilities
   into rating scales. Portfolio PD calibration, horizon
   mapping and explicit scale selection belong to record
   02 and any qualified consumer. Agency grades, hazard
   inputs and capital or impairment PDs have their own
   contracts.
   [Accepted rating boundaries](../core-conventions/019-credit-ratings.md#accepted-decision-generic-ratings-and-explicit-scorecard-scales),
   [PD and scales](02-pd-scales-and-rating-data.md).

10. **Reconcile the existing score calculators and scorecard.**
    Use one canonical Rust tagged scoring operation,
    `credit_score(request)`, for the existing multi-model
    route, retaining the typed calculators. Reconcile Rust,
    Python, WASM, checked JSON/pickle state, schema and
    generated types, parity, and the live credit-scorecard
    request, display, provenance and fixtures. Distinguish
    invalid inputs from computation errors in both host
    mappings. Show the selected model and only its available
    output fields. Keep any shared stable logistic primitive
    available to its existing model callers without changing
    their methodology.

11. **Qualify formula reproduction and actual use separately.**
    Check coefficient vectors and accounting bases, adjacent
    screening boundaries, SIZE scaling, flags and normalized
    earnings change, signed and extreme ratios, stable link
    outputs, and host restoration. A current credit use
    requires independent population, vintage, event and
    horizon qualification with as-of information, suitable
    missing-data treatment and out-of-sample discrimination
    and calibration evidence. The fixed historical formulas
    provide no such validation by themselves.

## Observed behavior and limitations

The [Altman calculators](../../../../finstack-quant/models/src/credit/scoring/altman.rs)
use a rounded public Z sales coefficient of `1.0`.
The [Ohlson calculator](../../../../finstack-quant/models/src/credit/scoring/ohlson.rs)
uses the Model 1 coefficient vector with a two-year module
description and approximate SIZE guidance. The
[Zmijewski calculator](../../../../finstack-quant/models/src/credit/scoring/zmijewski.rs)
uses the retained vector with a total-liabilities field.
Both links provide default probability zones. Input checks
do not validate the complete affine score before the result.

The [tagged report](../../../../finstack-quant/models/src/credit/scoring/report.rs)
maps native probabilities through the registry-default
master scale. The [score result](../../../../finstack-quant/models/src/credit/scoring/types.rs)
stores a free-form model label and derived fields. The
[credit scorecard](../../../../finstack-quant-ui/registry/models/components/credit-scorecard/credit-scorecard.tsx)
uses the report and displays that grade and central PD.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| Altman Z with sales/assets `2.991` and all other ratios zero | The original coefficient gives `2.988009` and Grey. The rounded coefficient gives `2.991` and Safe. |
| Altman Z″ ratios `(.2,.3,.15,.8)` | Z″ is `4.138`; the corresponding EM score is `7.388`. Both fall in their respective Safe screens. |
| Ohlson inputs in formula order `(12,.4,.2,.5,0,.08,.3,0,.1)` | Score `-4.83085` and native one-year model probability about `0.00791656352336`. |
| Ohlson assets USD 100 million and index 100 | SIZE is `ln(1000000)`. Substituting `ln(100)` would raise the score by about `3.748608531`. |
| Zmijewski with ROA `.1`, source debt/assets `.4` and current ratio `2` | Score `-2.5077`, native probability about `0.00607598949702`. Substituting a liabilities ratio of `.7` raises the score by `1.7037`. |
| Altman Z with `X1=7*2^1021`, `X2=-6*2^1021`, `X5=1` and other ratios zero | The exact decimal-coefficient sum is `0.999`. Evaluating isolated binary64 products overflows before cancellation. |

[Existing tests](../../../../finstack-quant/models/src/credit/scoring/tests.rs)
cover coefficients, zones and input errors. The independent
examples verify selected calculations. Runtime, host and
empirical qualification remain pending.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
