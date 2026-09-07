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

<a id="fed-k8-holidays"></a>

### Fed K.8 Holidays

- Board of Governors of the Federal Reserve System. *K.8 Holidays Observed by
  the Federal Reserve System*. Saturday/Sunday substitution rules for Fed
  holidays.

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

<a id="brigo-mercurio-2006-interest-rate-models"></a>

### Brigo Mercurio 2006 Interest Rate Models

- Brigo, D., and Mercurio, F. *Interest Rate Models — Theory and Practice*
  (2nd ed.). Springer Finance. Caps, floors, swaptions, Hull-White, and
  quanto-adjustment conventions.

<a id="hull-white-1990-pricing-ird"></a>

### Hull White 1990 Pricing Interest-Rate Derivatives

- Hull, J., and White, A. "Pricing Interest-Rate-Derivative Securities."
  *Review of Financial Studies*, 3(4), 573-592. One-factor Hull-White short-rate
  model used by HW1F calibration and tree pricers.

<a id="hull-white-1994-numerical-procedures"></a>

### Hull White 1994 Numerical Procedures

- Hull, J., and White, A. "Numerical Procedures for Implementing Term Structure
  Models I: Single-Factor Models." *Journal of Derivatives*. Trinomial-tree
  construction for the Hull-White short-rate model.

<a id="jamshidian-1989-bond-option"></a>

### Jamshidian 1989 Bond Option

- Jamshidian, F. "An Exact Bond Option Formula." *Journal of Finance*, 44(1),
  205-209. Closed-form zero-bond option used to decompose European swaptions
  under Hull-White.

<a id="isda-2021-definitions"></a>

### ISDA 2021 Interest Rate Derivatives Definitions

- International Swaps and Derivatives Association. *2021 ISDA Interest Rate
  Derivatives Definitions*. Overnight RFR compounded-in-arrears swap
  conventions.

<a id="bloomberg-swpm"></a>

### Bloomberg SWPM

- Bloomberg L.P. *SWPM* (Swap Manager) help and screen conventions. Vendor
  reference for IRS compounding labels, par-rate DV01, and production
  swaption par-par quoting used to reconcile `InterestRateSwap` pricing.

<a id="sadr-2009-irs"></a>

### Sadr 2009 Interest Rate Swaps

- Sadr, A. *Interest Rate Swaps and Their Derivatives: A Practitioner's
  Guide*. Wiley. Practitioner reference for swap mechanics, multi-curve
  discounting, and market quoting conventions.

<a id="andersen-piterbarg-xccy-mtm-reset"></a>

### MtM-Resetting Cross-Currency Swaps

- Andersen & Piterbarg, *Interest Rate Modeling Vol. III: Products and Risk Management*, §16.2 (Cross-Currency Swaps), §16.4 (MtM-Resetting Notionals).
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

<a id="ametrano-bianchetti-2013"></a>

### Ametrano Bianchetti 2013

- Ametrano, F. M., and Bianchetti, M. "Everything You Always Wanted to Know
  About Multiple Interest Rate Curve Bootstrapping but Were Afraid to Ask."
  Multi-curve discounting and forwarding used by basis-swap pricing.

<a id="fujii-shimada-takahashi-2010"></a>

### Fujii Shimada Takahashi 2010

- Fujii, M., Shimada, Y., and Takahashi, A. "A Note on Construction of
  Multiple Swap Curves with and without Collateral." Collateralized
  multi-curve construction.

<a id="sifma-mbs-standard-formulas"></a>

### SIFMA MBS Standard Formulas

- SIFMA. *Standard Formulas for the Analysis of Mortgage-Backed Securities
  and Other Related Securities* (2010 ed.). Weighted-average-life and related
  MBS time metrics (actual/365).

<a id="sifma-tba-good-delivery"></a>

### SIFMA TBA Good Delivery

- SIFMA. *Good Delivery Guidelines*. TBA allocation and pool-delivery
  conventions, including Section 3.2.

<a id="fabozzi-fixed-income-handbook"></a>

### Fabozzi Fixed Income Handbook

- Fabozzi, F. J. *The Handbook of Fixed Income Securities*. Practitioner
  reference for MBS weighted-average life and related mortgage analytics.

<a id="deacon-derry-mirfendereski-2004"></a>

### Deacon Derry Mirfendereski 2004

- Deacon, M., Derry, A., and Mirfendereski, D. *Inflation-Indexed Securities*
  (2nd ed.). Wiley. Index-linked bond mechanics, lagging, and inflation-curve
  construction.

<a id="kerkhof-2005"></a>

### Kerkhof 2005

- Kerkhof, J. "Inflation Derivatives Explained." Inflation-index lagging and
  inflation-derivative market conventions.

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

<a id="isda-2014-credit-definitions"></a>

### ISDA 2014 Credit Derivatives Definitions

- International Swaps and Derivatives Association. *2014 ISDA Credit
  Derivatives Definitions*. Credit events, successor, and settlement
  conventions used by CDS instruments.

<a id="jarrow-lando-turnbull-1997"></a>

### Jarrow Lando Turnbull 1997

- Jarrow, R. A., Lando, D., and Turnbull, S. M. "A Markov Model for the Term
  Structure of Credit Risk Spreads." *Review of Financial Studies*, 10(2),
  481-523. Rating-transition generator used by credit-migration primitives.

<a id="creditmetrics-1997"></a>

### CreditMetrics 1997

- Gupton, G. M., Finger, C. C., and Bhatia, M. *CreditMetrics — Technical
  Document*. J.P. Morgan. Credit-migration and portfolio credit-risk
  methodology.

<a id="israel-rosenthal-wei-2001"></a>

### Israel Rosenthal Wei 2001

- Israel, R., Rosenthal, J., and Wei, J. "Finding Generators for Markov Chains
  via Empirical Transition Matrices, with Applications to Credit Ratings."
  *Mathematical Finance*, 11(2), 245-265. Generator extraction from
  one-year transition matrices.

<a id="laurent-gregory-2005-factor-copulas"></a>

### Laurent Gregory 2005 Factor Copulas

- Laurent, J.-P., and Gregory, J. "Basket Default Swaps, CDOs and Factor
  Copulas." *Journal of Risk*, 7(4), 103-122. Factor-copula loss
  distributions for CDS tranches.

<a id="gibson-2004-synthetic-cdos"></a>

### Gibson 2004 Synthetic CDOs

- Gibson, M. S. "Understanding the Risk of Synthetic CDOs." *Finance and
  Economics Discussion Series* 2004-36, Federal Reserve Board. Tranche loss
  and mezzanine risk interpretation.

<a id="isda-cds-standard-model"></a>

### ISDA CDS Standard Model

- ISDA CDS Standard Model documentation and related ISDA credit-derivatives
  conventions. Use for hazard-rate, survival-probability, and CDS-style
  accrual/settlement references.

<a id="bloomberg-cds-model"></a>

### Bloomberg CDS Model

- Bloomberg L.P. Quantitative Analytics. *The Bloomberg CDS Model.* DOCS
  2057273 ⟨GO⟩. CDSW screen conventions for clean principal, extra-day final
  coupon, and par-spread annuity used by `CreditDefaultSwap`.

<a id="bloomberg-cdso"></a>

### Bloomberg CDSO

- Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.* DOCS
  2055833 ⟨GO⟩. Numerical-quadrature CDS-option model used by `CDSOption`.

<a id="hull-white-2000-cds"></a>

### Hull White 2000 CDS

- Hull, J. C., and White, A. "Valuing Credit Default Swaps I: No Counterparty
  Default Risk." *Journal of Derivatives*, 8(1), 29-40. Reduced-form CDS
  pricing without counterparty default.

<a id="sp-cds-indices-primer"></a>

### S&P CDS Indices Primer

- S&P Dow Jones Indices. *CDS Indices Primer*.
  <https://www.spglobal.com/spdji/en/landing/topic/credit-default-swap-cds-indices/>
  Clean-price strike quotation for CDX HY index options and the published
  factor/loss default-adjusted strike example (`107.0 -> 107.9874` with
  `f0 = 1.00`, `f = 0.99`, one 1%-weight default at 9.25% recovery), used as
  an independent unit fixture for the price-strike payoff algebra.

<a id="altman-1968"></a>

### Altman 1968

- Altman, E. I. "Financial Ratios, Discriminant Analysis and the Prediction of
  Corporate Bankruptcy." Original Altman Z-Score reference.

<a id="ohlson-1980"></a>

### Ohlson 1980

- Ohlson, J. A. "Financial Ratios and the Probabilistic Prediction of
  Bankruptcy." *Journal of Accounting Research*, 18(1), 109-131. Logistic
  O-score used by `ohlson_o_score`.

<a id="zmijewski-1984"></a>

### Zmijewski 1984

- Zmijewski, M. E. "Methodological Issues Related to the Estimation of
  Financial Distress Prediction Models." *Journal of Accounting Research*,
  22, 59-82. Probit distress model used by `zmijewski_score`.

<a id="diebold-li-2006"></a>

### Diebold Li 2006

- Diebold, F. X., and Li, C. "Forecasting the Term Structure of Government Bond
  Yields." Nelson-Siegel factor dynamics and yield-curve forecasting.

<a id="nelson-siegel-1987"></a>

### Nelson Siegel 1987

- Nelson, C. R., and Siegel, A. F. "Parsimonious Modeling of Yield Curves."
  *Journal of Business*, 60(4), 473-489. Four-parameter yield-curve
  parameterization used by `ParametricCurve`.

<a id="svensson-1994"></a>

### Svensson 1994

- Svensson, L. E. O. "Estimating and Interpreting Forward Interest Rates:
  Sweden 1992-1994." *NBER Working Paper* 4871. Six-parameter
  Nelson-Siegel-Svensson extension.

<a id="litterman-scheinkman-1991"></a>

### Litterman Scheinkman 1991

- Litterman, R., and Scheinkman, J. "Common Factors Affecting Bond Returns."
  *Journal of Fixed Income*, 1(1), 54-61. Level/slope/curvature PCA of the
  yield curve.

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

<a id="litterman-1996-hotspots"></a>

### Litterman 1996 Hot Spots

- Litterman, R. "Hot Spots and Hedges." *Journal of Portfolio Management*,
  Special Issue. Practitioner reference for identifying concentrated risk
  contributions inside a portfolio.

<a id="hallerbach-2003-decomposing-var"></a>

### Hallerbach 2003 Decomposing VaR

- Hallerbach, W. G. "Decomposing Portfolio Value-at-Risk: A General Analysis."
  *Journal of Risk*, 5(2). Historical and Euler-style VaR/ES contribution
  analysis used by position-level decomposers.

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

<a id="schuermann-2004-lgd"></a>

### Schuermann 2004 LGD

- Schuermann, T. "What Do We Know About Loss Given Default?" Wharton Financial
  Institutions Center Working Paper 04-01. Recovery-rate evidence by seniority.

<a id="vasicek-2002-loan-portfolio"></a>

### Vasicek 2002 Loan Portfolio

- Vasicek, O. A. "The Distribution of Loan Portfolio Value." *Risk*, 15(12),
  160-162. Asymptotic single-factor portfolio-loss distribution used in IRB
  PD mapping.

<a id="basel-ii-2006"></a>

### Basel II 2006

- Basel Committee on Banking Supervision. *International Convergence of
  Capital Measurement and Capital Standards: A Revised Framework*. IRB PD
  floors, EAD/CCF, and asset-correlation formulas.

<a id="lando-skodeberg-2002"></a>

### Lando Skodeberg 2002

- Lando, D., and Skodeberg, T. M. "Analyzing Rating Transitions and Rating
  Drift with Continuous Observations." *Journal of Banking & Finance*,
  26(2-3), 423-444. Continuous-time rating-migration generators.

<a id="duffie-singleton-1999"></a>

### Duffie Singleton 1999

- Duffie, D., and Singleton, K. J. "Modeling Term Structures of Defaultable
  Bonds." Reduced-form intensity modeling of defaultable discounting.

<a id="lando-1998"></a>

### Lando 1998

- Lando, D. "On Cox Processes and Credit Risky Securities." Cox-process
  (doubly stochastic) default intensity used by stochastic credit engines.

<a id="richard-roll-1989"></a>

### Richard Roll 1989

- Richard, S. F., and Roll, R. "Prepayments on Fixed-Rate Mortgage-Backed
  Securities." *Journal of Portfolio Management*, 15(3), 9-14. Refinancing,
  seasoning, and burnout prepayment model.

<a id="schwartz-torous-1989"></a>

### Schwartz Torous 1989

- Schwartz, E. S., and Torous, W. N. "Prepayment and the Valuation of
  Mortgage-Backed Securities." Empirical prepayment and MBS valuation.

<a id="moodys-rating-symbols"></a>

### Moody's Rating Symbols

- Moody's Investors Service. *Rating Symbols and Definitions*. Distressed
  exchange and default-event definitions used in liability-management
  classification.

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
  Market Risk* (BCBS d457), published 14 January 2019; corrected version
  published 25 February 2019. Consolidated as Basel Framework chapter
  **MAR21**, "Standardised approach: sensitivities-based method", version
  effective 1 January 2023 (text incorporates the FAQs published 5 July 2024
  and 23 March 2026). Canonical reference for FRTB standardized-approach
  delta, vega, curvature, default-risk, and residual-risk add-on calculations.
  Per-parameter paragraph and table citations, together with the recorded
  deviations between the implementation and the standard, live in the module
  docs of `finstack-quant/margin/src/regulatory/frtb/params/` and in
  `finstack-quant/margin/data/margin/README.md`.

<a id="gregory-xva-challenge"></a>

### Gregory XVA Challenge

- Gregory, J. *The xVA Challenge*. Practitioner reference for exposure
  simulation, collateral, CVA, DVA, and FVA workflows.

<a id="green-xva"></a>

### Green XVA

- Green, A. *XVA: Credit, Funding and Capital Valuation Adjustments*.
  Practitioner reference for bilateral XVA decomposition and funding-adjustment
  conventions.

## Liquidity And Market Microstructure

<a id="roll-1984"></a>

### Roll 1984

- Roll, R. "A Simple Implicit Measure of the Effective Bid-Ask Spread in an
  Efficient Market." Serial-covariance estimator used by
  `roll_effective_spread`.

<a id="amihud-2002"></a>

### Amihud 2002

- Amihud, Y. "Illiquidity and Stock Returns: Cross-Section and Time-Series
  Effects." Average absolute-return-to-volume ratio used by
  `amihud_illiquidity`.

<a id="bangia-1999-lvar"></a>

### Bangia 1999 LVaR

- Bangia, A., Diebold, F. X., Schuermann, T., and Stroughair, J. D. "Modeling
  Liquidity Risk, With Implications for Traditional Market Risk Measurement
  and Management." Liquidity-adjusted VaR used by `lvar_bangia`.

<a id="almgren-chriss-2000"></a>

### Almgren Chriss 2000

- Almgren, R., and Chriss, N. "Optimal Execution of Portfolio Transactions."
  Permanent and temporary impact decomposition used by `almgren_chriss_impact`.

<a id="kyle-1985"></a>

### Kyle 1985

- Kyle, A. S. "Continuous Auctions and Insider Trading." Price-impact
  coefficient estimated by `kyle_lambda`.

<a id="hasbrouck-2007"></a>

### Hasbrouck 2007

- Hasbrouck, J. *Empirical Market Microstructure*. Oxford University Press.
  Bid-ask spread, mid-price, and related microstructure conventions used by
  `LiquidityProfile`.

<a id="aifmd-liquidity-management"></a>

### AIFMD Liquidity Management

- Directive 2011/61/EU (AIFMD), Article 16, and ESMA, *Guidelines on liquidity
  stress testing in UCITS and AIFs* (ESMA34-39-897). Industry practice for
  days-to-liquidate bucketing used by `LiquidityTier`.

## Volatility, Options, And Smile Models

<a id="black-1976"></a>

### Black 1976

- Black, F. "The Pricing of Commodity Contracts." The standard reference for the
  Black (1976) forward-style option pricing model.

<a id="black-scholes-1973"></a>

### Black Scholes 1973

- Black, F., and Scholes, M. "The Pricing of Options and Corporate Liabilities."
  Canonical European option pricing model on a non-dividend-paying underlying.

<a id="merton-1973"></a>

### Merton 1973

- Merton, R. C. "Theory of Rational Option Pricing." Continuous-dividend-yield
  extension of Black-Scholes used by the spot-style closed-form primitives.

<a id="garman-kohlhagen-1983"></a>

### Garman Kohlhagen 1983

- Garman, M. B., and Kohlhagen, S. W. "Foreign Currency Option Values." FX
  option formula recovered by treating the foreign rate as a continuous yield.

<a id="merton-1976-jump"></a>

### Merton 1976 Jump Diffusion

- Merton, R. C. "Option Pricing When Underlying Stock Returns Are
  Discontinuous." Jump-diffusion characteristic function used by the COS
  pricer.

<a id="madan-carr-chang-1998"></a>

### Madan Carr Chang 1998

- Madan, D. B., Carr, P. P., and Chang, E. C. "The Variance Gamma Process and
  Option Pricing." Variance-gamma characteristic function used by the COS
  pricer.

<a id="fang-oosterlee-2008"></a>

### Fang Oosterlee 2008

- Fang, F., and Oosterlee, C. W. "A Novel Pricing Method for European Options
  Based on Fourier-Cosine Series Expansions." COS method used by the Fourier
  option primitives.

<a id="andersen-2008-heston-qe"></a>

### Andersen 2008 Heston QE

- Andersen, L. "Simple and Efficient Simulation of the Heston Stochastic
  Volatility Model." Quadratic-exponential discretization used by the Heston
  Monte Carlo pricers.

<a id="reiner-rubinstein-1991"></a>

### Reiner Rubinstein 1991

- Reiner, E., and Rubinstein, M. "Breaking Down the Barriers." Continuous-
  monitoring barrier option formulae.

<a id="kemna-vorst-1990"></a>

### Kemna Vorst 1990

- Kemna, A. G. Z., and Vorst, A. C. F. "A Pricing Method for Options Based on
  Average Asset Values." Exact geometric-average Asian option formula.

<a id="turnbull-wakeman-1991"></a>

### Turnbull Wakeman 1991

- Turnbull, S. M., and Wakeman, L. M. "A Quick Algorithm for Pricing European
  Average Options." Moment-matching arithmetic-average Asian approximation.

<a id="conze-viswanathan-1991"></a>

### Conze Viswanathan 1991

- Conze, A., and Viswanathan. "Path Dependent Options: The Case of Lookback
  Options." Closed-form lookback option formulae.

<a id="goldman-sosin-gatto-1979"></a>

### Goldman Sosin Gatto 1979

- Goldman, M. B., Sosin, H. B., and Gatto, M. A. "Path Dependent Options: Buy
  at the Low, Sell at the High." *Journal of Finance*, 34(5), 1111-1127.
  Continuous lookback option formulae.

<a id="broadie-glasserman-kou-1997"></a>

### Broadie Glasserman Kou 1997

- Broadie, M., Glasserman, P., and Kou, S. G. "A Continuity Correction for
  Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349. Discrete-
  monitoring barrier continuity correction.

<a id="levy-1992-asian-options"></a>

### Levy 1992 Asian Options

- Levy, E. "Pricing European Average Rate Currency Options." *Journal of
  International Money and Finance*, 11(5), 474-491. Moment-matching arithmetic
  Asian approximation.

<a id="curran-1994-asian-options"></a>

### Curran 1994 Asian Options

- Curran, M. "Valuing Asian and Portfolio Options by Conditioning on the
  Geometric Mean Price." *Management Science*, 40(12), 1705-1711. Geometric-
  conditioning Asian approximation.

<a id="haug-2007-option-formulas"></a>

### Haug 2007 Option Pricing Formulas

- Haug, E. G. *The Complete Guide to Option Pricing Formulas* (2nd ed.).
  McGraw-Hill. Practitioner catalogue of closed-form exotic formulae.

<a id="carr-madan-1999-fft"></a>

### Carr Madan 1999 FFT

- Carr, P., and Madan, D. "Option Valuation Using the Fast Fourier Transform."
  *Journal of Computational Finance*, 2(4), 61-73. Fourier option pricing used
  alongside the COS method.

<a id="albrecher-2007-little-heston-trap"></a>

### Albrecher 2007 Little Heston Trap

- Albrecher, H., Mayer, P., Schoutens, W., and Tistaert, J. "The Little Heston
  Trap." *Wilmott Magazine*, January 2007. Numerically stable Heston
  characteristic-function branch.

<a id="demeterfi-1999-volatility-swaps"></a>

### Demeterfi 1999 Volatility Swaps

- Demeterfi, K., Derman, E., Kamal, M., and Zou, J. "More Than You Ever Wanted
  to Know About Volatility Swaps." Goldman Sachs Quantitative Strategies
  Research Notes. Replication formula for variance swaps.

<a id="rebonato-2004-volatility-correlation"></a>

### Rebonato 2004 Volatility And Correlation

- Rebonato, R. *Volatility and Correlation: The Perfect Hedger and the Fox*
  (2nd ed.). Wiley. Swaption volatility, SABR, and LMM correlation.

<a id="lord-koekkoek-vandijk-2010"></a>

### Lord Koekkoek Van Dijk 2010

- Lord, R., Koekkoek, R., and Van Dijk, D. "A Comparison of Biased Simulation
  Schemes for Stochastic Volatility Models." *Quantitative Finance*, 10(2),
  177-194. Heston discretization bias comparison.

<a id="broadie-kaya-2006-exact-heston"></a>

### Broadie Kaya 2006 Exact Heston

- Broadie, M., and Kaya, Ö. "Exact Simulation of Stochastic Volatility and
  Other Affine Jump Diffusion Processes." *Operations Research*, 54(2),
  217-231. Exact Heston simulation (not the QE scheme).

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

<a id="hagan-2003-cms-convexity"></a>

### Hagan 2003 CMS Convexity

- Hagan, P. S. "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
  *Wilmott Magazine*, March 2003. CMS convexity adjustment under the annuity
  measure.

<a id="bayer-friz-gatheral-2016"></a>

### Bayer Friz Gatheral 2016

- Bayer, C., Friz, P., and Gatheral, J. "Pricing under rough volatility."
  *Quantitative Finance*, 16(6), 887-904. Rough Bergomi and forward-variance
  curve inputs.

<a id="gatheral-jaisson-rosenbaum-2018"></a>

### Gatheral Jaisson Rosenbaum 2018

- Gatheral, J., Jaisson, T., and Rosenbaum, M. "Volatility is rough."
  *Quantitative Finance*, 18(6), 933-949. Empirical roughness of volatility
  and the Hurst exponent near 0.1.

<a id="el-euch-rosenbaum-2019"></a>

### El Euch Rosenbaum 2019

- El Euch, O., and Rosenbaum, M. "The characteristic function of rough Heston
  models." *Mathematical Finance*, 29(1), 3-38. Rough Heston dynamics and
  characteristic function.

<a id="bennedsen-lunde-pakkanen-2017"></a>

### Bennedsen Lunde Pakkanen 2017

- Bennedsen, M., Lunde, A., and Pakkanen, M. S. "Hybrid scheme for Brownian
  semistationary processes." *Finance and Stochastics*, 21(4), 931-965.
  Hybrid simulation of Volterra / rough-volatility kernels.

<a id="mccrickerd-pakkanen-2018"></a>

### McCrickerd Pakkanen 2018

- McCrickerd, R., and Pakkanen, M. S. "Turbocharging Monte Carlo pricing for
  the rough Bergomi model." *Quantitative Finance*, 18(11), 1877-1886.

<a id="schwartz-smith-2000"></a>

### Schwartz Smith 2000

- Schwartz, E., and Smith, J. E. "Short-Term Variations and Long-Term Dynamics
  in Commodity Prices." *Management Science*, 46(7), 893-911. Two-factor
  commodity spot model.

<a id="kirk-1995"></a>

### Kirk 1995

- Kirk, E. "Correlation in the Energy Markets." *Managing Energy Price Risk*.
  Kirk approximation for commodity spread options.

<a id="tsiveriotis-fernandes-1998"></a>

### Tsiveriotis Fernandes 1998

- Tsiveriotis, K., and Fernandes, C. "Valuing Convertible Bonds with Credit
  Risk." *Journal of Fixed Income*, 8(2), 95-102. Split-equity/debt
  convertible pricing.

<a id="ayache-forsyth-vetzal-2003"></a>

### Ayache Forsyth Vetzal 2003

- Ayache, E., Forsyth, P. A., and Vetzal, K. R. "Valuation of Convertible
  Bonds with Credit Risk." *Journal of Derivatives*, 11(1), 9-29.

<a id="whaley-2009-vix"></a>

### Whaley 2009 VIX

- Whaley, R. E. "Understanding the VIX." *Journal of Portfolio Management*,
  35(3), 98-105. Volatility-index construction and VIX futures/options.

<a id="cboe-vix-white-paper"></a>

### CBOE VIX White Paper

- CBOE. *VIX White Paper* and VIX futures/options contract specifications.
  Replication formula and listed vol-index derivative conventions.

<a id="carr-wu-2006"></a>

### Carr Wu 2006

- Carr, P., and Wu, L. "A Tale of Two Indices." *Journal of Derivatives*,
  13(3), 13-29. Model-free vs VIX-style volatility indices.

<a id="carr-lee-2009"></a>

### Carr Lee 2009

- Carr, P., and Lee, R. "Volatility Derivatives." *Annual Review of Financial
  Economics*, 1, 319-339. Variance/volatility swaps and VIX-style options.

<a id="cont-tankov-2004"></a>

### Cont Tankov 2004

- Cont, R., and Tankov, P. *Financial Modelling with Jump Processes*.
  Characteristic functions and cumulants used by Fourier (COS) pricing.

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

<a id="finger-2002-creditgrades"></a>

### Finger 2002 CreditGrades

- Finger, C. C., Finkelstein, V., Pan, G., Lardy, J.-P., Ta, T., and Tierney, J.
  *CreditGrades Technical Document*. RiskMetrics Group. Source of the
  uncertain-barrier first-passage survival approximation and its lognormal
  recovery dispersion.

<a id="crosbie-bohn-2003-kmv"></a>

### Crosbie Bohn 2003 KMV

- Crosbie, P. and Bohn, J. *Modeling Default Risk*. Moody's KMV. Source of the
  physical-measure distance-to-default and EDF mapping, and of the
  short-term-debt-plus-half-long-term-debt default point.

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

<a id="joe-kuo-2008-sobol"></a>

### Joe Kuo 2008 Sobol

- Joe, S., and Kuo, F. Y. "Constructing Sobol Sequences with Better
  Two-Dimensional Projections." *SIAM Journal on Scientific Computing*, 30(5),
  2635-2654. Direction numbers used by the Sobol generator.

<a id="sobol-1967"></a>

### Sobol 1967

- Sobol, I. M. "Distribution of points in a cube and approximate evaluation of
  integrals." *USSR Computational Mathematics and Mathematical Physics*, 7(4),
  86-112. Original Sobol sequence.

<a id="owen-1995-scrambling"></a>

### Owen 1995 Scrambling

- Owen, A. B. "Randomly Permuted (t,m,s)-Nets and (t,s)-Sequences." Monte Carlo
  and Quasi-Monte Carlo Methods in Scientific Computing, 299-317. Owen
  scrambling of Sobol nets.

<a id="box-muller-1958"></a>

### Box Muller 1958

- Box, G. E. P., and Muller, M. E. "A Note on the Generation of Random Normal
  Deviates." Polar/Box-Muller transform for Gaussian sampling.

<a id="salmon-2011-philox"></a>

### Salmon 2011 Philox

- Salmon, J. K., Moraes, M. A., Dror, R. O., and Shaw, D. E. "Parallel Random
  Numbers: As Easy as 1, 2, 3." *SC '11*. Counter-based Philox generator used
  for splittable Monte Carlo streams.

<a id="o-neill-2014-pcg"></a>

### O'Neill 2014 PCG

- O'Neill, M. E. "PCG: A Family of Simple Fast Space-Efficient Statistically
  Good Algorithms for Random Number Generation." Permuted congruential
  generators.

<a id="mandelbrot-van-ness-1968"></a>

### Mandelbrot Van Ness 1968

- Mandelbrot, B., and Van Ness, J. "Fractional Brownian Motions, Fractional
  Noises and Applications." *SIAM Review*, 10(4), 422-437. Fractional Brownian
  motion and the Hurst exponent.

<a id="in-t-hout-welfert-2009"></a>

### In 't Hout Welfert 2009

- In 't Hout, K. J., and Welfert, B. D. "Unconditional stability of second-order
  ADI schemes applied to multi-dimensional diffusion equations with mixed
  derivative terms." Modified Craig-Sneyd ADI used by 2D PDE engines.

<a id="broadie-detemple-1996"></a>

### Broadie Detemple 1996

- Broadie, M., and Detemple, J. "American Option Valuation: New Bounds,
  Approximations, and a Comparison of Existing Methods." *Review of Financial
  Studies*, 9(4), 1211-1250. Tree/Richardson extrapolation for American
  options.

<a id="halton-1960"></a>

### Halton 1960

- Halton, J. H. "On the efficiency of certain quasi-random sequences of points
  in evaluating multi-dimensional integrals." *Numerische Mathematik*, 2(1),
  84-90. Halton low-discrepancy sequence used by multi-start calibration.

<a id="gilli-maringer-schumann-2011"></a>

### Gilli Maringer Schumann 2011

- Gilli, M., Maringer, D., and Schumann, E. *Numerical Methods and Optimization
  in Finance*. Multi-start and global-search practice for model calibration.

<a id="levenshtein-1966"></a>

### Levenshtein 1966

- Levenshtein, V. I. "Binary codes capable of correcting deletions, insertions,
  and reversals." Edit-distance used by identifier suggestion matching.

<a id="wagner-fischer-1974"></a>

### Wagner Fischer 1974

- Wagner, R. A., and Fischer, M. J. "The String-to-String Correction Problem."
  *Journal of the ACM*, 21(1), 168-173. Dynamic-programming edit distance.

<a id="meeus-1991"></a>

### Meeus 1991

- Meeus, J. *Astronomical Algorithms*. Easter-date algorithms used by holiday
  calendars.

<a id="gillespie-1977"></a>

### Gillespie 1977

- Gillespie, D. T. "Exact Stochastic Simulation of Coupled Chemical Reactions."
  *Journal of Physical Chemistry*, 81(25), 2340-2361. Exact continuous-time
  Markov-chain simulation used by rating-path sampling.

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

<a id="campisi-2000"></a>

### Campisi 2000

- Campisi, S. "Primer on Fixed Income Performance Attribution." *Journal of
  Portfolio Management*, 26(4), 14-25. Canonical carry / treasury / spread /
  selection decomposition for fixed-income attribution.

<a id="ben-dor-2007-dts"></a>

### Ben Dor 2007 DTS

- Ben Dor, A., Dynkin, L., Hyman, J., Houweling, P., van Leeuwen, E., and
  Penninga, O. "DTS (Duration Times Spread)." *Journal of Portfolio
  Management*, 33(2), 77-100. Source of the DTS convention, which expresses
  credit exposure as `D * s` against a *relative* spread change. Cited by
  `finstack-quant/portfolio/src/fi_attribution.rs` to record why Campisi
  attribution offers no DTS mode: the return leg is an algebraic identity
  (`-(D * s)(ds / s)` = `-D * ds`), so given a realized spread change the two
  conventions produce the same number. DTS earns its keep only in the
  volatility and hedge-ratio legs, where `D * s` is a standalone risk quantity
  multiplied by an empirically more stable relative spread volatility - risk
  and hedging surfaces, not ex-post attribution.

<a id="dynkin-hyman-vankudre-1998"></a>

### Dynkin Hyman Vankudre 1998

- Dynkin, L., Hyman, J., and Vankudre, P. "Attribution of Portfolio
  Performance Relative to an Index." Lehman Brothers Fixed Income Research,
  March 1998. Appendix B source for bucketing a reference (e.g. Treasury)
  universe into duration cells, averaging returns within each cell, and
  interpolating/extrapolating empty cells - used by
  `finstack-quant/portfolio/src/excess_return.rs` to build the duration-matched
  base return curve for credit excess return calculations. Appendix A source
  for the hierarchical curve/sector/selection grid decomposition (duration
  cell x sector, with an out-of-benchmark fallback) - used by
  `finstack-quant/portfolio/src/grid_attribution.rs`.

<a id="jeet-partani-2023"></a>

### Jeet Partani 2023

- Jeet, V., & Partani, A. (2023). "Brinson-Style Attribution over Continuous
  Factors." *The Journal of Portfolio Management*, Quantitative Special Issue
  2023, 216-223. Appendix A source for the equality-constrained least-squares
  correction (`f = f̂ + λg`) used by
  `finstack-quant/analytics/src/regression.rs::constrained_least_squares` to
  enforce `w'Xf = w'r` on top of an unconstrained OLS factor-return fit, for
  factor-Brinson attribution.

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
  modified Sharpe ratio: mean excess return divided by modified VaR measured
  at the corresponding horizon.

<a id="jpmorgan1996RiskMetrics"></a>

### J.P. Morgan RiskMetrics 1996

- J.P. Morgan/Reuters. *RiskMetrics Technical Document* (4th ed.). Canonical
  practitioner reference for parametric Value-at-Risk conventions.

<a id="ledoitwolf2004"></a>

### Ledoit Wolf 2004

- Ledoit, O., & Wolf, M. (2004). "A well-conditioned estimator for
  large-dimensional covariance matrices." *Journal of Multivariate Analysis*,
  88(2), 365–411. Canonical reference for identity-target covariance
  shrinkage with analytic optimal intensity, implemented in
  `finstack_quant_core::math::linalg::ledoit_wolf_shrinkage` and consumed by
  `CovarianceStrategy::LedoitWolf` in the credit factor-model calibrator.

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

## Corporate Valuation And Structured Products

<a id="koller-valuation"></a>

### Koller Valuation

- Koller, T., Goedhart, M., and Wessels, D. *Valuation: Measuring and Managing
  the Value of Companies*. Enterprise-value, LBO, and sources-and-uses
  analysis.

<a id="rosenbaum-pearl-2020"></a>

### Rosenbaum Pearl 2020

- Rosenbaum, J., and Pearl, J. *Investment Banking: Valuation, LBOs, M&A, and
  IPOs*. LBO entry valuation, sources and uses, and exit-return analysis.

<a id="overhaus-2007-equity-derivatives"></a>

### Overhaus 2007 Equity Derivatives

- Overhaus, M. et al. *Equity Derivatives: Theory and Applications*. Wiley.
  Autocallable and related equity-structured-product mechanics.

## Expected Credit Loss And Accounting Impairment

<a id="ifrs-9-impairment"></a>

### IFRS 9 Impairment

- IFRS Foundation. *IFRS 9 Financial Instruments*, Section 5.5. Expected-credit-
  loss staging and measurement used by `EclEngine`.

<a id="asc-326-cecl"></a>

### ASC 326 CECL

- Financial Accounting Standards Board. *ASC 326-20 — Financial Instruments:
  Credit Losses*. Current expected credit loss (CECL) measurement used by
  `CeclEngine`.

<a id="bcbs-2015-ecl-guidance"></a>

### BCBS 2015 ECL Guidance

- Basel Committee on Banking Supervision. "Guidance on credit risk and
  accounting for expected credit losses" (2015). Supervisory interpretation of
  lifetime ECL and staging.
