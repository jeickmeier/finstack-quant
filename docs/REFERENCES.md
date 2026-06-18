# Finstack Quant Documentation References

This file provides stable anchors for canonical references used across the
`finstack-quant` crates. Public Rust, Python, and WASM documentation should link here
when an API implements a market convention, pricing model, numerical method, or
risk calculation with a standard reference.

## Usage

- Prefer links of the form `docs/REFERENCES.md#anchor-name` in rustdoc
  `# References` sections.
- Use the most specific anchor that matches the algorithm or convention.
- If a public API relies on market practice rather than a single paper, cite the
  closest industry standard first, then a practitioner text if needed.

## Day Count And Business-Day Conventions

<a id="isda-2006-definitions"></a>

### ISDA 2006 Definitions

- International Swaps and Derivatives Association. *2006 ISDA Definitions*.
  Sections covering day-count fractions, business-day conventions, and schedule
  adjustments.

<a id="icma-rule-book"></a>

### ICMA Rule Book

- International Capital Market Association. *ICMA Rule Book*. Bond-market
  conventions for accrued interest and irregular coupon handling, including
  Actual/Actual (ICMA/ISMA) style calculations.

<a id="iso-8601"></a>

### ISO 8601

- International Organization for Standardization. *ISO 8601 Date and Time
  Format*. Canonical reference for calendar, week-date, and period notation.

## Curves, Discounting, And Interest Rates

<a id="hull-options-futures"></a>

### Hull Options Futures

- Hull, J. C. *Options, Futures, and Other Derivatives*. Standard reference for
  discounting, forwards, swaps, and foundational derivatives pricing.

<a id="andersen-piterbarg-interest-rate-modeling"></a>

### Andersen Piterbarg Interest Rate Modeling

- Andersen, L. B. G., and Piterbarg, V. V. *Interest Rate Modeling*. Multi-curve
  discounting, term-structure construction, and interest-rate modeling
  conventions.

<a id="andersen-piterbarg-xccy-mtm-reset"></a>

### MtM-Resetting Cross-Currency Swaps

- Andersen & Piterbarg, *Interest Rate Modeling Vol. III: Products and Risk Management*, §16.2 (Cross-Currency Swaps), §16.4 (MtM-Resetting Notionals).
- See `docs/superpowers/specs/2026-05-10-xccy-mtm-reset-design.md` for the implementation spec.
- The implementation uses CIP forward FX without FX-rate correlation convexity; suitable
  for vanilla G10 basis curve construction and position pricing. Per market convention
  (matching QuantLib's `MtMCrossCurrencyBasisSwap`), the rebalancing cashflow is emitted
  on the resetting leg only — the constant leg's principal-and-coupon schedule is unchanged.

<a id="arrc-sofr-users-guide"></a>

### ARRC SOFR Users Guide

- Alternative Reference Rates Committee. *SOFR: A User's Guide*. Practitioner
  reference for SOFR floating-rate conventions, including in-arrears
  compounding, observation shifts, lookbacks, and payment lags.

<a id="ecb-estr-methodology"></a>

### ECB ESTR Methodology

- European Central Bank. *Euro Short-Term Rate (€STR) Methodology and Policies*.
  Reference for €STR publication and euro overnight rate conventions.

<a id="boe-sonia-key-features"></a>

### BoE SONIA Key Features

- Bank of England. *SONIA Key Features and Policies*. Reference for SONIA
  publication and sterling overnight rate conventions.

<a id="boj-tona"></a>

### BoJ TONA

- Bank of Japan. *Uncollateralized Overnight Call Rate*. Reference for the Tokyo
  overnight average rate and yen overnight money-market conventions.

<a id="hagan-west-monotone-convex"></a>

### Hagan West Monotone Convex

- Hagan, P. S., and West, G. "Interpolation Methods for Curve Construction."
  Canonical reference for monotone-convex interpolation used in yield-curve
  construction.

<a id="tuckman-serrat-fixed-income"></a>

### Tuckman Serrat Fixed Income

- Tuckman, B., and Serrat, A. *Fixed Income Securities*. Standard text for
  key-rate risk, DV01, and fixed-income hedging intuition.

## Credit, Correlation, And Portfolio Risk

<a id="isda-cds-standard-model"></a>

### ISDA CDS Standard Model

- ISDA CDS Standard Model documentation and related ISDA credit-derivatives
  conventions. Use for hazard-rate, survival-probability, and CDS-style
  accrual/settlement references.

<a id="altman-1968"></a>

### Altman 1968

- Altman, E. I. "Financial Ratios, Discriminant Analysis and the Prediction of
  Corporate Bankruptcy." Original Altman Z-Score reference.

<a id="diebold-li-2006"></a>

### Diebold Li 2006

- Diebold, F. X., and Li, C. "Forecasting the Term Structure of Government Bond
  Yields." Nelson-Siegel factor dynamics and yield-curve forecasting.

<a id="mcneil-frey-embrechts-qrm"></a>

### McNeil Frey Embrechts QRM

- McNeil, A. J., Frey, R., and Embrechts, P. *Quantitative Risk Management*.
  Canonical reference for VaR, Expected Shortfall, and portfolio risk
  interpretation.

<a id="meucci-risk-and-asset-allocation"></a>

### Meucci Risk And Asset Allocation

- Meucci, A. *Risk and Asset Allocation*. Reference for factor models, covariance
  aggregation, and exposure-based portfolio risk decomposition.

<a id="tasche-2008-capital-allocation"></a>

### Tasche 2008 Capital Allocation

- Tasche, D. "Capital Allocation to Business Units and Sub-Portfolios: the Euler
  Principle." Canonical reference for Euler allocation of portfolio risk across
  factors or sub-portfolios.

<a id="li-2000-gaussian-copula"></a>

### Li 2000 Gaussian Copula

- Li, D. X. "On Default Correlation: A Copula Function Approach." *Journal of
  Fixed Income*, 9(4), 43-54. Canonical reference for one-factor Gaussian
  copula modeling of portfolio default correlation.

<a id="demarta-mcneil-2005-t-copula"></a>

### Demarta McNeil 2005 T Copula

- Demarta, S., and McNeil, A. J. "The t Copula and Related Copulas."
  *International Statistical Review*, 73(1), 111-129. Canonical reference for
  multivariate Student-t copulas and lower-tail dependence.

<a id="hull-predescu-white-2005"></a>

### Hull Predescu White 2005

- Hull, J., Predescu, M., and White, A. "The Valuation of Correlation-Dependent
  Credit Derivatives Using a Structural Model." Practitioner reference for
  Student-t and correlation-sensitive credit-derivative valuation.

<a id="andersen-sidenius-2005-rfl"></a>

### Andersen Sidenius 2005 RFL

- Andersen, L., and Sidenius, J. "Extensions to the Gaussian Copula: Random
  Recovery and Random Factor Loadings." *Journal of Credit Risk*. Canonical
  reference for stochastic recovery and random-factor-loading extensions to the
  Gaussian copula.

<a id="andersen-sidenius-basu-2003"></a>

### Andersen Sidenius Basu 2003

- Andersen, L., Sidenius, J., and Basu, S. "All Your Hedges in One Basket."
  *Risk*, November 2003. Practitioner reference for multi-factor basket and
  bespoke CDO correlation modeling.

<a id="hull-white-2004-cdo"></a>

### Hull White 2004 CDO

- Hull, J., and White, A. "Valuation of a CDO and an n-th to Default CDS
  Without Monte Carlo Simulation." Canonical reference for analytical
  correlation-product valuation with Gaussian-style latent-factor models.

<a id="altman-et-al-2005-recovery"></a>

### Altman Et Al 2005 Recovery

- Altman, E., Brady, B., Resti, A., and Sironi, A. "The Link between Default
  and Recovery Rates: Theory, Empirical Evidence, and Implications."
  *Journal of Business*, 78(6). Canonical reference for the empirical
  relationship between default clustering and recovery outcomes.

<a id="krekel-stumpp-2006-correlation-products"></a>

### Krekel Stumpp 2006 Correlation Products

- Krekel, M., and Stumpp, P. "Pricing Correlation Products: CDOs."
  Practitioner reference for tranche and stochastic-recovery calibration
  conventions in credit correlation products.

## Margin, Collateral, And XVA

<a id="isda-2002-master-agreement"></a>

### ISDA 2002 Master Agreement

- International Swaps and Derivatives Association. *2002 ISDA Master Agreement*.
  Canonical reference for close-out netting and default-management terms used in
  OTC derivatives netting sets.

<a id="isda-vm-csa-2016"></a>

### ISDA 2016 VM CSA

- International Swaps and Derivatives Association. *Credit Support Annex for
  Variation Margin (VM CSA)*. Standard reference for regulatory VM collateral
  terms, threshold conventions, and margin-call mechanics.

<a id="isda-im-csa-2018"></a>

### ISDA 2018 IM CSA

- International Swaps and Derivatives Association. *Credit Support Deed and
  Credit Support Annex for Initial Margin*. Standard reference for segregated IM
  documentation and collateral terms for uncleared derivatives.

<a id="isda-simm"></a>

### ISDA SIMM

- International Swaps and Derivatives Association. *Standard Initial Margin
  Model (SIMM) Methodology*. Canonical reference for SIMM risk classes, buckets,
  risk weights, correlations, concentration thresholds, and margin aggregation.

<a id="bcbs-iosco-uncleared-margin"></a>

### BCBS IOSCO Uncleared Margin

- Basel Committee on Banking Supervision and International Organization of
  Securities Commissions. *Margin Requirements for Non-Centrally Cleared
  Derivatives*. Standard reference for regulatory IM and VM requirements,
  including the schedule-based fallback methodology.

<a id="bcbs-279-saccr"></a>

### BCBS 279 SA-CCR

- Basel Committee on Banking Supervision. *The Standardised Approach for
  Measuring Counterparty Credit Risk Exposures* (BCBS 279). Canonical reference
  for Effective EPE and counterparty-credit-risk exposure terminology.

<a id="bcbs-frtb-minimum-capital-requirements"></a>

### BCBS FRTB Minimum Capital Requirements

- Basel Committee on Banking Supervision. *Minimum Capital Requirements for
  Market Risk* (BCBS d457). Canonical reference for FRTB standardized-approach
  delta, vega, curvature, default-risk, and residual-risk add-on calculations.

<a id="gregory-xva-challenge"></a>

### Gregory XVA Challenge

- Gregory, J. *The xVA Challenge*. Practitioner reference for exposure
  simulation, collateral, CVA, DVA, and FVA workflows.

<a id="green-xva"></a>

### Green XVA

- Green, A. *XVA: Credit, Funding and Capital Valuation Adjustments*.
  Practitioner reference for bilateral XVA decomposition and funding-adjustment
  conventions.

## Volatility, Options, And Smile Models

<a id="black-1976"></a>

### Black 1976

- Black, F. "The Pricing of Commodity Contracts." The standard reference for the
  Black (1976) forward-style option pricing model.

<a id="bachelier-1900"></a>

### Bachelier 1900

- Bachelier, L. *The Theory of Speculation*. Canonical reference for normal-model
  option pricing.

<a id="gatheral-volatility-surface"></a>

### Gatheral Volatility Surface

- Gatheral, J. *The Volatility Surface*. Canonical reference for implied-volatility
  parameterizations, total variance, and smile dynamics.

<a id="gatheral-2004-svi"></a>

### Gatheral 2004 SVI

- Gatheral, J. "A Parsimonious Arbitrage-Free Implied Volatility
  Parameterization." Standard SVI slice reference.

<a id="gatheral-jacquier-2014-svi"></a>

### Gatheral Jacquier 2014 SVI

- Gatheral, J., and Jacquier, A. "Arbitrage-Free SVI Volatility Surfaces."
  Follow-on reference for SVI no-arbitrage conditions.

<a id="hagan-2002-sabr"></a>

### Hagan 2002 SABR

- Hagan, P. S., Kumar, D., Lesniewski, A., and Woodward, D. "Managing Smile
  Risk." Canonical SABR reference.

<a id="heston-1993"></a>

### Heston 1993

- Heston, S. L. "A Closed-Form Solution for Options with Stochastic Volatility."
  Canonical Heston-model reference.

<a id="merton-1974"></a>

### Merton 1974

- Merton, R. C. "On the Pricing of Corporate Debt: The Risk Structure of
  Interest Rates." Canonical structural credit model reference.

<a id="o-kane-2008"></a>

### O Kane 2008

- O'Kane, D. *Modelling Single-name and Multi-name Credit Derivatives*.
  Practitioner reference for CreditGrades and CDS valuation conventions.

<a id="dupire-1994"></a>

### Dupire 1994

- Dupire, B. "Pricing with a Smile." Canonical local-volatility density reference.

<a id="clark-fx-options"></a>

### Clark FX Options

- Clark, I. *Foreign Exchange Option Pricing*. Reference for FX volatility
  conventions and smile construction.

<a id="wystup-fx-options"></a>

### Wystup FX Options

- Wystup, U. *FX Options and Structured Products*. Reference for delta-based FX
  volatility quoting and smile construction.

## Numerical Methods, Statistics, And Randomness

<a id="higham-accuracy-and-stability"></a>

### Higham Accuracy And Stability

- Higham, N. J. *Accuracy and Stability of Numerical Algorithms*. Canonical
  reference for floating-point error analysis and numerically stable algorithms.

<a id="press-numerical-recipes"></a>

### Press Numerical Recipes

- Press, W. H. et al. *Numerical Recipes*. Practical reference for root finding,
  integration, interpolation, and Monte Carlo techniques.

<a id="glasserman-2004-monte-carlo"></a>

### Glasserman 2004 Monte Carlo

- Glasserman, P. *Monte Carlo Methods in Financial Engineering*. Canonical
  reference for Monte Carlo scenario generation, tail-risk estimation, and
  variance-aware simulation practice.

<a id="golub-van-loan-matrix-computations"></a>

### Golub Van Loan Matrix Computations

- Golub, G. H., and Van Loan, C. F. *Matrix Computations*. Canonical reference
  for Cholesky factorization, covariance-matrix numerics, and matrix
  conditioning diagnostics.

<a id="welford-1962"></a>

### Welford 1962

- Welford, B. P. "Note on a Method for Calculating Corrected Sums of Squares and
  Products." Canonical one-pass variance reference.

<a id="kahan-1965"></a>

### Kahan 1965

- Kahan, W. "Further Remarks on Reducing Truncation Errors." Canonical reference
  for compensated summation.

<a id="brent-1973"></a>

### Brent 1973

- Brent, R. P. *Algorithms for Minimization Without Derivatives*. Canonical
  reference for Brent's method of bracketed root-finding combining bisection,
  secant, and inverse quadratic interpolation.

<a id="de-boor-splines"></a>

### De Boor Splines

- de Boor, C. *A Practical Guide to Splines*. Canonical reference for B-spline
  and cubic-spline interpolation, knot insertion, and end-condition handling.

<a id="fritsch-carlson-1980"></a>

### Fritsch Carlson 1980

- Fritsch, F. N., and Carlson, R. E. "Monotone Piecewise Cubic Interpolation."
  Canonical reference for shape-preserving (monotone) cubic Hermite
  interpolation.

## Performance Analytics, Portfolio Construction, And Risk Reporting

<a id="grinoldKahn1999ActivePortfolio"></a>

### Grinold Kahn 1999 Active Portfolio

- Grinold, R. C., and Kahn, R. N. *Active Portfolio Management*. Canonical
  practitioner reference for tracking error, information ratio, and
  benchmark-relative performance measurement.

<a id="brinson-fachler-1985"></a>

### Brinson Fachler 1985

- Brinson, G. P., Hood, L. R., and Beebower, G. L. "Determinants of Portfolio
  Performance." Foundation for Brinson-Fachler attribution.

<a id="carino-1999"></a>

### Carino 1999

- Carino, D. R. "Combining Attribution Effects Over Time." Multi-period
  linking for Brinson-style attribution.

<a id="fama-french-1993"></a>

### Fama French 1993

- Fama, E. F., and French, K. R. "Common Risk Factors in the Returns on Stocks
  and Bonds." Canonical reference for multi-factor equity return regressions.

<a id="treynor1965"></a>

### Treynor 1965

- Treynor, J. L. "How to Rate Management of Investment Funds." Canonical
  reference for the Treynor ratio and beta-based performance evaluation.

<a id="modigliani1997"></a>

### Modigliani 1997

- Modigliani, F., and Modigliani, L. "Risk-Adjusted Performance." Canonical
  reference for M-squared (Modigliani-Modigliani) performance reporting.

<a id="sharpe1966"></a>

### Sharpe 1966

- Sharpe, W. F. "Mutual Fund Performance." Canonical reference for the Sharpe
  ratio.

<a id="sortinoVanDerMeer1991"></a>

### Sortino Van Der Meer 1991

- Sortino, F. A., and van der Meer, R. "Downside Risk." Canonical reference for
  downside deviation and the Sortino ratio.

<a id="keatingShadwick2002"></a>

### Keating Shadwick 2002

- Keating, C., and Shadwick, W. F. "A Universal Performance Measure." Canonical
  reference for the Omega ratio.

<a id="schwager2012"></a>

### Schwager 2012

- Schwager, J. D. *Hedge Fund Market Wizards*. Common practitioner reference
  for the gain-to-pain ratio in hedge fund and CTA performance reporting.

<a id="gregoriou2003"></a>

### Gregoriou Gueyie 2003

- Gregoriou, G. N., and Gueyie, J.-P. "Risk-Adjusted Performance of Funds of
  Hedge Funds Using a Modified Sharpe Ratio." Canonical reference for the
  modified Sharpe ratio.

<a id="jpmorgan1996RiskMetrics"></a>

### J.P. Morgan RiskMetrics 1996

- J.P. Morgan/Reuters. *RiskMetrics Technical Document* (4th ed.). Canonical
  practitioner reference for parametric Value-at-Risk conventions.

<a id="artzner1999CoherentRisk"></a>

### Artzner 1999 Coherent Risk

- Artzner, P., Delbaen, F., Eber, J.-M., and Heath, D. "Coherent Measures of
  Risk." Canonical reference for Expected Shortfall as a coherent risk measure.

<a id="joanesGill1998"></a>

### Joanes Gill 1998

- Joanes, D. N., and Gill, C. A. "Comparing Measures of Sample Skewness and
  Kurtosis." Canonical reference for bias-corrected sample skewness and
  kurtosis estimators.

<a id="cornishFisher1937"></a>

### Cornish Fisher 1937

- Cornish, E. A., and Fisher, R. A. "Moments and Cumulants in the Specification
  of Distributions." Canonical reference for the Cornish-Fisher expansion.

<a id="bollerslev1986"></a>

### Bollerslev 1986

- Bollerslev, T. "Generalized Autoregressive Conditional Heteroskedasticity."
  Canonical reference for the symmetric GARCH(1,1) volatility model.

<a id="nelson1991"></a>

### Nelson 1991

- Nelson, D. B. "Conditional Heteroskedasticity in Asset Returns: A New
  Approach." Canonical reference for EGARCH volatility dynamics.

<a id="glosten1993"></a>

### Glosten Jagannathan Runkle 1993

- Glosten, L. R., Jagannathan, R., and Runkle, D. E. "On the Relation between
  the Expected Value and the Volatility of the Nominal Excess Return on Stocks."
  Canonical reference for GJR-GARCH asymmetric volatility dynamics.

<a id="ljungBox1978"></a>

### Ljung Box 1978

- Ljung, G. M., and Box, G. E. P. "On a Measure of Lack of Fit in Time Series
  Models." Canonical reference for the Ljung-Box serial-correlation test.

<a id="engle1982Arch"></a>

### Engle 1982 ARCH

- Engle, R. F. "Autoregressive Conditional Heteroscedasticity with Estimates of
  the Variance of United Kingdom Inflation." Canonical reference for ARCH and
  ARCH-LM testing.

<a id="chekhlov2005"></a>

### Chekhlov Uryasev Zabarankin 2005

- Chekhlov, A., Uryasev, S., and Zabarankin, M. "Drawdown Measure in Portfolio
  Optimization." Canonical reference for Conditional Drawdown at Risk.

<a id="martinUlcer1987"></a>

### Martin 1987 Ulcer Index

- Martin, P. G. "The Ulcer Index." Canonical practitioner reference for the
  Ulcer Index and related Martin ratio usage.

<a id="youngCalmar1991"></a>

### Young 1991 Calmar

- Young, T. W. "Calmar Ratio: A Smoother Tool." Practitioner reference for the
  Calmar ratio.

<a id="kestner1996"></a>

### Kestner 1996

- Kestner, L. N. *Quantitative Trading Strategies*. Practitioner reference for
  Sterling ratio conventions.

<a id="burke1994"></a>

### Burke 1994

- Burke, G. "A Sharper Sharpe Ratio." Practitioner reference for Burke-style
  drawdown-adjusted performance ratios.
