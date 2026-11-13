# Finstack Quant Documentation References

This file provides stable anchors for canonical references used across the
`finstack-quant` crates. Public Rust, Python, and WASM documentation should link here
when an API implements a market convention, pricing model, numerical method, or
risk calculation with a standard reference.

A reference explains a method; its presence here does not approve the method
for every instrument or caller. The
[financial methodology inventory and decisions](decisions/financial/README.md)
record adoption rationale, applicable scope, selection behavior, limitations,
and implementation and validation evidence.

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

<a id="uk-dmo-gilt-accrued-interest"></a>

### UK DMO Gilt Accrued Interest

- UK Debt Management Office. [Formulae for Calculating Gilt Prices from Yields](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf),
  fourth edition18December2024, sectionTHREE(1)(i), printed17; subsequent pages
  contain separate stub/indexation cases. [About Gilts](https://www.dmo.gov.uk/responsibilities/gilt-market/about-gilts/)
  explains its last-cum boundary and settlement restrictions. Verified for
  CASHFLOWS-013; these are product clauses, not a universal accrued-interest,
  rounding or principal-entitlement rule.

<a id="lse-fixed-interest-cum-ex-2024"></a>

### LSE Fixed-Interest Cum and Ex Rules — 2024

- London Stock Exchange. [Rules effective5February2024](https://docs.londonstockexchange.com/sites/default/files/documents/rules-of-the-london-stock-exchange-effective-5-february-2024.pdf),
  printed52, rules3110.1–3110.3. The verified frozen version distinguishes
  gilt/non-gilt date equality; it does not certify current regulatory compliance
  or the library's generic quotation operator.

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

### Displaced LMM and terminal-bond pricing

- Brace, Gatarek and Musiela derive the market model for simple
  forward rates. Denson and Joshi give the displaced diffusion and
  its terminal-measure proportional drift in equations 2.1 and 2.2.
  [Brace, Gatarek and Musiela](https://onlinelibrary.wiley.com/doi/10.1111/1467-9965.00028),
  [Denson and Joshi](https://fbe.unimelb.edu.au/__data/assets/pdf_file/0011/2591831/201.pdf).
- Hunter, Jäckel and Joshi average the drift at the start and at the
  predictor, reusing the same innovations. The proposed variant
  holds loadings and evolution eligibility fixed within each exact
  interval. Beveridge, Denson and Joshi discuss displacement and
  drift approximation.
  [Original predictor-corrector paper](https://citeseerx.ist.psu.edu/document?doi=3e26dc596f3d649c9bf1e525ade5f52c59db4b1b&repid=rep1&type=pdf),
  [Displaced-LMM discretization study](https://fbe.unimelb.edu.au/__data/assets/pdf_file/0011/2591885/171.pdf).
- Glasserman and Zhao study discretization of bond-price ratios to
  preserve discrete martingale properties. The proposed predictor-
  corrector acts on forward rates and can bias deflated bond moments
  at finite step sizes. A simple forward determines a bond ratio
  between its two tenor maturities.
  [Glasserman and Zhao](https://business.columbia.edu/faculty/research/arbitrage-free-discretization-lognormal-forward-libor-and-swap-rate-models),
  [Glasserman and Wang](https://www.dam.brown.edu/people/huiwang/research/box1.pdf).

<a id="hull-white-1990-pricing-ird"></a>

### Hull White 1990 Pricing Interest-Rate Derivatives

- Hull, J., and White, A. "Pricing Interest-Rate-Derivative Securities."
  *Review of Financial Studies*, 3(4), 573-592 (1990).
  [DOI: 10.1093/rfs/3.4.573](https://doi.org/10.1093/rfs/3.4.573).
  One-factor Hull-White context for calibration and tree pricers; it does not
  by itself qualify CORE-NUMERICS-21's finite-range step-kernel implementation,
  schedule conventions or model/financial error budget.

### Checked Hull–White analytic kernels and payment measures

- Hull and White give the extended Vasicek bond and bond-option law.
  The scheduled variance and exact zero-mean-reversion conventions are
  local model choices in
  [accepted record 23](decisions/financial/models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures).
  [Pricing Interest-Rate-Derivative Securities](https://doi.org/10.1093/rfs/3.4.573)
- Henrard's Theorems 2 and 3 derive separate delayed-payment adjustments
  for continuous accumulation and discrete resets in one-factor HJM.
  [Overnight indexed swaps and floating coupons](https://econwpa.ub.uni-muenchen.de/econ-wp/fin/papers/0402/0402008.pdf)
- Kirikos and Novak give the futures bias derivation. QuantLib implements
  the quote-dependent constant-sigma benchmark with accrual equal to its
  time gap. The selected model takes contractual accrual explicitly.
  [Convexity Conundrums](https://www.powerfinance.com/convexity/),
  [QuantLib Hull–White implementation](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/models/shortrate/onefactormodels/hullwhite.cpp)
- The term caplet's ATM Normal quote uses the analytic price equivalence in
  Choi et al., section 4.2. [A Black–Scholes user's guide to the Bachelier model](https://arxiv.org/pdf/2104.08686)

### Hull–White rate clock, curve fit and term projection

- QuantLib's Hull–White dynamics separate the centered OU state from
  the deterministic shift. Its constant-sigma fitting parameter gives
  the corresponding instantaneous-forward and variance correction.
  [QuantLib Hull–White dynamics and fitting parameter](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/models/shortrate/onefactormodels/hullwhite.hpp)
- Henrard describes the absolutely continuous discount function and
  the one-factor Gaussian bond law. The local schedule and integral
  in [record 24](decisions/financial/models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)
  use the same integrated Gaussian law.
  [Efficient swaptions price in Hull–White one factor model](https://arxiv.org/abs/0901.1776)
- Henrard defines the index period with its own fixing, effective and
  maturity dates and contractual accrual. Definitions 1–3 give the
  multiplicative gross spread. Record 24 selects its deterministic
  case with the projection factor source of CORE-MARKET-02.
  [Multi-curves Framework with Stochastic Spread](https://quant.opengamma.io/Multi-Curves-Stochastic-Spread-STIR-Futures-Options-OpenGamma.pdf)

### Exact joint Hull–White paths and sampling

- Ostrovski derives the conditional Gaussian law of the centered
  endpoint and integrated state, with the deterministic curve
  shift in the discount factor. The scheduled square-root
  composition and event dispatch in
  [record 25](decisions/financial/models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling)
  extend the constant-sigma law to the accepted coefficient schedule.
  [Efficient and Exact Simulation of the Hull-White Model](https://www.researchgate.net/publication/262336369_Efficient_and_Exact_Simulation_of_the_Hull-White_Model)
- Salmon et al. specify Philox's counter-based construction. The
  local stream, midpoint uniform and ordered two-normal assignment
  are recorded in the proposal.
  [Parallel Random Numbers: As Easy as 1, 2, 3](https://www.thesalmons.org/john/random123/papers/random123sc11.pdf)
- Glasserman, section 4.2, derives the antithetic estimator as
  the mean of independent pair averages. The record selects
  the core Student-t mean interval on those pricing observations.
  [Monte Carlo Methods in Financial Engineering](https://doi.org/10.1007/978-0-387-21617-1)

### Hull–White endpoint paths and curve-fit boundaries

- Gillespie derives the exact Gaussian OU transition. The accepted
  endpoint model uses signed piecewise-constant mean levels and the
  common scheduled state-noise law on the actual interval.
  [Exact numerical simulation of the Ornstein–Uhlenbeck process and its integral](https://journals.aps.org/pre/abstract/10.1103/PhysRevE.54.2084).
- Hull and White describe fitting a short-rate model to its initial
  curve. Ostrovski derives the centered state and deterministic shift
  with a joint Gaussian rate integral. The accepted dated fit and
  cash discount law are in analytic records 24 and 25.
  [Using Hull–White Interest Rate Trees](https://www-2.rotman.utoronto.ca/~hull/downloadablepublications/USETREE.pdf),
  [Efficient and Exact Simulation of the Hull-White Model](https://www.researchgate.net/publication/262336369_Efficient_and_Exact_Simulation_of_the_Hull-White_Model).

### Fixed-budget Monte Carlo pricing and estimation

- Glasserman's foundations connect derivative pricing to expectations under
  the appropriate measure. The accepted generic engine contract admits a single
  deterministic discount factor and relies on the caller to supply the
  corresponding payoff representation and measure.
  [Monte Carlo Methods in Financial Engineering, Foundations](https://link.springer.com/chapter/10.1007/978-0-387-21617-1_1).
- Hammersley and Morton introduce antithetic coupling. In the accepted
  engine the complete pair average is one independent observation for
  uncertainty calculations.
  [A New Monte Carlo Technique](https://www.cambridge.org/core/journals/mathematical-proceedings-of-the-cambridge-philosophical-society/article/abs/new-monte-carlo-technique-antithetic-variates/69A9BBEDC6A4F1B1AF7E0764CD422E15).
- Student derives the small-sample mean statistic for normal observations.
  Howard et al. develop time-uniform confidence sequences under separate
  assumptions. The generic pricing contract selects a fixed-count
  Student-t interval and removes width-driven sequential stopping.
  [The Probable Error of a Mean](https://ocw.mit.edu/courses/esd-86-models-data-and-inference-for-socio-technical-systems-spring-2007/resources/gosset_1908/),
  [Time-uniform confidence sequences](https://arxiv.org/abs/1810.08240).

### Paired Monte Carlo spot sensitivities

- Broadie and Glasserman describe paired finite-difference repricing,
  its finite-bump error, and European option derivatives in their
  simulation comparison and Appendices B and C.
  [Estimating Security Price Derivatives Using Simulation](https://www.columbia.edu/~mnb2/broadie/Assets/bg-ms-1996.pdf)
- Glasserman and Yao explain how common random numbers couple
  comparisons and identify conditions for variance reduction.
  Record 25 fixes the draw assignment across scenarios and estimates
  uncertainty from the resulting complete groups.
  [Some Guidelines and Guarantees for Common Random Numbers](https://business.columbia.edu/sites/default/files-efs/pubfiles/4261/glasserman_yao_guidelines.pdf)
- Fornberg derives derivative weights for arbitrarily spaced grids.
  The accepted gamma is the three-node quadratic case. The accepted
  delta is the slope of the two-endpoint linear interpolant.
  [Generation of Finite Difference Formulas on Arbitrarily Spaced Grids](https://www.colorado.edu/amath/sites/default/files/attached-files/mathcomp_88_fd_formulas.pdf)

### GBM likelihood-ratio delta and vega

- Broadie and Glasserman derive terminal likelihood-ratio delta
  and vega in Proposition 4, equations (37)–(38), and joint
  Asian path scores in Proposition 6, equations (48)–(49).
  Record 26 selects the same Gaussian score law at the
  actual grid intervals with frozen deterministic carry.
  [Estimating Security Price Derivatives Using Simulation](https://www.columbia.edu/~mnb2/broadie/Assets/bg-ms-1996.pdf)

### Paired control variates and independent pilots

- Glynn and Szechtman derive the fixed-coefficient adjustment
  from a known control expectation and the variance-minimizing
  covariance-to-variance ratio. Their estimated-coefficient
  limit results concern asymptotic efficiency.
  [Some New Perspectives on the Method of Control Variates](https://web.stanford.edu/~glynn/papers/2002/GSzechtman02.pdf)
- Nelson's Section 6 explains how fitting on an independent
  pilot makes the coefficient independent of the pricing
  sample. Record 27 selects this split for the dated
  arithmetic Asian pricer and calculates uncertainty from
  the held-out adjusted observations.
  [Control Variate Remedies](https://users.iems.northwestern.edu/~nelsonb/Publications/ControlVariateRemedies.pdf)

### Longstaff-Schwartz holder stopping and independent policy pricing

- Longstaff and Schwartz use regression on decision-time state
  to estimate the discounted value of continuing on in-the-money
  paths. Their Section 3 compares the fitted rule on the original
  simulation and on an independent simulation. The accepted holder
  contract selects the independent valuation route.
  [Valuing American Options by Simulation](https://galton.uchicago.edu/~mykland/346W05/Longstaff.pdf)
- Andersen and Broadie give the population lower bound for an
  admissible maximizing stopping policy in equation (3).
  [Primal-Dual Simulation Algorithm](https://business.columbia.edu/sites/default/files-efs/pubfiles/1233/primal_dual_ms_2004.pdf)
- Woo, Liu and Choi analyze look-ahead from using the same
  observations to fit continuation and estimate policy value.
  [Leave-One-Out Least Squares Monte Carlo Algorithm](https://arxiv.org/abs/1810.02071)

### Continuation regression bases and rank

- Golub and Reinsch derive SVD least squares and discuss loss of
  accuracy from forming the Gram matrix. Record 24 accepts a
  bounded decomposition and full-rank admission on the scaled
  design, with a caller-selected threshold.
  [Singular Value Decomposition and Least Squares Solutions](https://people.duke.edu/~hpgavin/SystemID/References/Golub%2BReinsch-NM-1970.pdf)
- The finite series for ordinary Laguerre gives one polynomial of
  each degree, with nonzero leading coefficient. Longstaff and
  Schwartz use exponentially weighted Laguerre functions in their
  put example. The unweighted series spans the same polynomial space
  as powers of the same maximum degree.
  [NIST DLMF §18.5](https://dlmf.nist.gov/18.5),
  [Longstaff and Schwartz, sections 1 and 3](https://galton.uchicago.edu/~mykland/346W05/Longstaff.pdf)
- Glasserman and Yu analyze how basis degree and path count jointly
  affect worst-case convergence. Algebraic rank alone provides no
  exercise-value accuracy guarantee.
  [Number of Paths Versus Number of Basis Functions](https://arxiv.org/abs/math/0503556)

### Checked issuer-call policy training and pricing

- Longstaff and Schwartz estimate conditional continuation by
  regression on decision-time state, then apply the fitted rules
  to independent paths in their sample comparison. Their
  path-dependent cancellable swap includes current notional in
  the state. [Valuing American Options by Simulation](https://people.math.ethz.ch/~hjfurrer/teaching/LongstaffSchwartzAmericanOptionsLeastSquareMonteCarlo.pdf)
- Andersen and Broadie, equation (3), give the population
  inequality for an admissible maximizing stopping policy.
  [Record 26](decisions/financial/models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing)
  applies the same argument to issuer minimization of holder
  cash. [Primal-Dual Simulation Algorithm](https://business.columbia.edu/sites/default/files-efs/pubfiles/1234/1234.pdf)
- Glasserman and Yu analyze the dependence of regression
  accuracy on sample count and basis size. Record 26's eligible
  base-group and numerical rank gates are local admission rules.
  [Number of Paths Versus Number of Basis Functions](https://arxiv.org/pdf/math/0503556)

<a id="ou-state-variance-kernel"></a>

### OU state variance kernel

- [QuantLib's official Ornstein–Uhlenbeck implementation](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/processes/ornsteinuhlenbeckprocess.cpp)
  gives the constant-coefficient state-noise variance
  `sigma^2*(1-exp(-2*kappa*dt))/(2*kappa)` and its zero-rate limit.
  CORE-NUMERICS-21 derives the scheduled/anchored kernel by segment integration;
  QuantLib's own small-rate cutoff, implementation arithmetic and error policy
  are not adopted. This is not integrated-short-rate or bond-option variance
  without the consuming model's additional transformation.

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

<a id="fixing-publication-timing"></a>

### Fixing Publication Timing and Revisions

- New York Fed, [Additional Information about Reference Rates](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates),
  checked2026-11-09: “Calculation Methodology for the SOFR Averages and Index”
  distinguishes SOFR value date from its next-business-day publication;
  “Details on Publication and Revisions for the Treasury Repo Reference Rates”
  distinguishes morning publication and possible same-day afternoon revisions.
- ECB, [Euro Foreign Exchange Reference Rates](https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/index.en.html),
  checked2026-11-09: the usual concertation and publication times differ, EUR is
  the base currency, and the reference rates have an informational purpose.
  These are source-specific knowledge/quotation examples, not authority for a
  universal realized-forward scenario policy, date window or conflict tolerance.

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
  and Other Related Securities*. General reference for weighted-average-life
  and related MBS time metrics; specific uses require their owning clauses.
- [SIFMA-hosted chapter SF](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf),
  The Bond Market Association: sectionB.2(a)–(c), printedSF5–7, for mortality
  conversion, PSA and AGE/MONTH; sectionB.4, printedSF13–14, for ABS. These formula
  pages are dated02/01/99; the PDF includes later page updates, so neither a
  uniform whole-edition date nor the prior generic2010 attribution is verified
  for this source. SectionC.4–C.5, printedSF20–22, supplies SDA profile, final
  liquidation-window and collateral-applicability boundaries for CASHFLOWS-011.
  Page-level support for010/011 does not qualify every calculation named
  elsewhere in this register or the complete product cashflow models.
- For average life, G.1(a), printedSF48, defines receipt time on30/360; G.1(c),
  SF49, uses actual receipt of principal. E.2, SF44, discusses actual cash delay.
  H.1, SF56–57, distinguishes signed principal payments for GPM/ARM from
  positive-payment Z-bond treatment. These product clauses do not establish
  universal ACT/365F economic-date WAL and are not primary authority for the
  explicitly selected house repayment-life clock in CASHFLOWS-014.

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
  2055833 ⟨GO⟩. Numerical-quadrature CDS-option model used by `CdsOption`.

<a id="hull-white-2000-cds"></a>

### Hull White 2000 CDS

- Hull, J. C., and White, A. "Valuing Credit Default Swaps I: No Counterparty
  Default Risk." *Journal of Derivatives*, 8(1), 29-40. Reduced-form CDS
  pricing without counterparty default.
- Verified version: [April2000 author working paper hosted by NYU Stern](https://w4.stern.nyu.edu/finance/docs/WP/2000/pdf/wpa00021.pdf).
  Section1/equations2–5 specifies claim/recovery and default-time
  loss assumptions; section2/equation9 prices CDS payoff/accrual under
  stated independence and no-counterparty-default assumptions. Its q(t) is
  unconditional default density, not hazard intensity. These clauses do not
  establish cashflow-kind credit treatment, inferred opening exposure or FIFO
  scheduled-payment recovery as generic reporting conventions. Verification
  of this working-paper version does not certify every downstream credit model.

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

- Altman's 1968 equation (1) uses a sales coefficient of 0.999.
  Its first four predictors use percentage units. Section V
  gives the historical 1.81 and 2.99 screen boundaries.
  [Financial Ratios, Discriminant Analysis and the Prediction of Corporate Bankruptcy](https://www.calctopia.com/papers/Altman1968.pdf)
- Altman's later chapter gives the revised Z′, Z″ and EM
  formulas. Table 1.6 describes the private-firm screening
  boundaries. Table 1.7 reports historical means by bond
  rating for the EMS analysis.
  [Revisiting the Z-Score and ZETA Models](https://www.blackwellpublishing.com/content/BPL_images/Content_Store/Sample_Chapter/9780631225638/altman.pdf)

<a id="ohlson-1980"></a>

### Ohlson 1980

- Ohlson's Section 5 and Table 4 define the one-year Model 1
  and the prior-year GNP index with 1968=100. Section 6 describes
  the historical classification error tradeoff.
  [Financial Ratios and the Probabilistic Prediction of Bankruptcy](https://www.jstor.org/stable/2490395)

<a id="zmijewski-1984"></a>

### Zmijewski 1984

- Zmijewski defines the debt/assets, return on assets and current
  ratio inputs on page 66 and studies sample selection in probit
  distress models. Shumway reports the retained coefficient
  vector in Table 4 with his replication's own ratio basis.
  [Methodological Issues Related to the Estimation of Financial Distress Prediction Models](https://www.jstor.org/stable/2490859),
  [Forecasting Bankruptcy More Accurately](https://www.tylergshumway.org/Shumway-ForecastingBankruptcyAccurately-2001.pdf)

<a id="diebold-li-2006"></a>

### Diebold Li 2006

- Diebold, F. X., and Li, C. "Forecasting the Term Structure of Government Bond
  Yields." Nelson-Siegel factor dynamics and yield-curve forecasting.

### Checked Nelson–Siegel factors and VAR forecasts

- Diebold and Li, sections 3.2–3.3, use fixed decay for the
  cross-sectional factor fit and direct horizon-specific
  regressions in the forecast comparison.
  [Record 27](decisions/financial/models-analytic/diebold-li-var.md#accepted-decision-for-checked-nelsonsiegel-factors-and-var-forecasts)
  uses the same factor representation with an iterated one-step VAR.
  [Forecasting the Term Structure of Government Bond Yields](https://elearning.unito.it/sme/pluginfile.php/473145/course/section/81686/DieboldLi_JoE06.pdf)
- Lütkepohl gives the finite-horizon innovation covariance and
  marginal Gaussian intervals in section 3.5. The local interval
  projects that covariance onto the fitted Nelson–Siegel signal.
  [Applied Time Series Econometrics](https://new.mmf.lnu.edu.ua/wp-content/uploads/2018/03/lutkepohl_time_series.pdf)
- Süli, section 3.7.2, gives the Cayley transformation and the
  Routh–Hurwitz criterion used for the strict cubic stability
  test. The exact dyadic evaluation is a local numerical choice.
  [Numerical Solution of Ordinary Differential Equations](https://people.maths.ox.ac.uk/suli/nsodes.pdf)

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
  *Journal of Fixed Income*, 1(1), 54-61. Empirical bond-return factors
  commonly interpreted as level, slope and curvature.
  [Common Factors Affecting Bond Returns](https://doi.org/10.3905/jfi.1991.692347)

### Checked yield-change PCA and scenarios

- Golub and Reinsch relate the singular values and right singular
  vectors of a data matrix to its Gram matrix and explain the
  numerical cost of forming the Gram matrix directly.
  [Record 28](decisions/financial/models-analytic/yield-pca.md#accepted-decision-for-checked-yield-change-pca-and-scenarios)
  applies this to centered yield changes with sample-covariance scale.
  [Singular Value Decomposition and Least Squares Solutions](https://people.duke.edu/~hpgavin/SystemID/References/Golub%2BReinsch-NM-1970.pdf)
- Yu, Wang and Samworth relate estimated subspace sensitivity to
  eigenvalue separation and extend their result to singular vectors.
  The accepted sign anchor and variance-retention rules govern each
  fitted snapshot.
  [A useful variant of the Davis–Kahan theorem for statisticians](https://arxiv.org/abs/1405.0680)

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

- Vasicek derives a conditional default probability under a
  Gaussian systematic factor and independent idiosyncratic
  normal shocks. Averaging the conditional probability over
  the factor recovers the unconditional probability.
  [The Distribution of Loan Portfolio Value](https://www.bancoinvest.pt/docs/default-source/fundamentais-risco-credito/distributionloanportfoliovalue.pdf)

<a id="basel-ii-2006"></a>

### Basel II 2006

- Basel Committee on Banking Supervision. *International Convergence of
  Capital Measurement and Capital Standards: A Revised Framework*.
  The historical paragraph 285 applies its 0.03% PD floor to
  specified one-year corporate and bank inputs. A later CRE32
  edition uses different floors and scope. Record 02 leaves
  application with the regulatory owner.
  [Basel II comprehensive version](https://www.bis.org/publ/bcbs128.pdf),
  [Basel CRE32 dated edition](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/32/inforce/2023-01-01/published/2020-03-27)

### Historical CLO rating factors

- The Moody's CLO methodology dated May 15, 2024 uses ten-year
  idealized default rates scaled by 10,000 for its stated rating
  inputs. Exhibit 1 includes Ca and C at 10,000. The table
  comparison supports historical provenance only. A live
  structured-credit use requires a qualified selected edition and
  its own treatment of unrated or missing inputs.
  [Moody's Global Approach to Rating Collateralized Loan Obligations](https://ratings.moodys.com/api/rmc-documents/420962)

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
  This entry names the family only. The implementation embeds and tests the
  historical v2.6 edition (December 2023); see the
  [margin data guide](../finstack-quant/margin/data/margin/README.md#simmv1json)
  and the [v2.6 provenance record](../finstack-quant/margin/tests/fixtures/simm_v26_provenance.md).

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

<a id="forward-option-formulas-and-numeraire"></a>

### Forward Option Formulas and Numeraire

- OpenGamma Strata [BlackFormulaRepository](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/BlackFormulaRepository.html)
  and [NormalFormulaRepository](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/NormalFormulaRepository.html),
  inspected2026-11-09, define forward/unit-numeraire prices and raw derivatives;
  actual numeraire multiplication remains external to those formulas.
- The official [shifted-Black caplet implementation](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/capfloor/ShiftedBlackIborCapletFloorletExpiryStrikeVolatilities.java)
  delegates regular price/Greek formulas using shifted F/K at fixed displacement.
  This supports formula comparison, not its surface/date conventions or numerical
  thresholds, floors and reference substitutions.
- [DiscountingSwapLegPricer.pvbp](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swap/DiscountingSwapLegPricer.html)
  explicitly distinguishes its value-at-rate1/physical-annuity quantity from a
  literal one-basis-point coupon value. Actual units and notional must be
  qualified; a PV01 label alone does not determine the multiplier.

<a id="black-1976"></a>

### Black 1976

- Black, F. "The Pricing of Commodity Contracts." The standard reference for the
  Black (1976) forward-style option pricing model.

<a id="bsm-formula-derivative-coordinates"></a>

### BSM Formula and Derivative Coordinates

- OpenGamma Strata, [BlackScholesFormulaRepository API](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/impl/option/BlackScholesFormulaRepository.html)
  and [official implementation](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/impl/option/BlackScholesFormulaRepository.java),
  inspected2026-11-09 for regular interior price/derivative formulas. Its cost
  of carry is b=r−q; domestic rho varies r and b together holding q fixed,
  and dividend/foreign-rate rho is minus carryRho. Theta is minus the remaining-
  time derivative. These coordinates support formula comparison, not adoption
  of Strata's thresholds, reference-value substitutes, tail truncation or
  a project-wide accuracy policy. MODELS-ANALYTIC-01 selects native derivative
  units separately; historical paper citations below remain bibliographic.

<a id="black-scholes-1973"></a>

### Black Scholes 1973

- Black, F., and Scholes, M. "The Pricing of Options and Corporate Liabilities."
  Canonical European option pricing model on a non-dividend-paying underlying.
  [Original paper](https://doi.org/10.1086/260062).

<a id="merton-1973"></a>

### Merton 1973

- Merton, R. C. "Theory of Rational Option Pricing." Continuous-dividend-yield
  extension of Black-Scholes used by the spot-style closed-form primitives.

### Checked CRR and Leisen–Reimer lattices

- Cox, Ross and Rubinstein derive strict risk-neutral support,
  the branch probabilities and discounted exercise recursion.
  [Option Pricing: A Simplified Approach](https://bpb-us-w2.wpmucdn.com/u.osu.edu/dist/7/36891/files/2017/07/CRR79-1yy8av8.pdf)
- Leisen and Reimer, section 4, use odd step counts and
  Peizer–Pratt inversion for strike-centered constructions.
  [Record 001](decisions/financial/models-numerical/equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices)
  selects method 2 and checked complementary-weight arithmetic.
  The paper distinguishes European convergence analysis from
  its early-exercise results.
  [Binomial Models for Option Valuation](https://wiwi.uni-bonn.de/sfb303/papers/1995/b/bonnsfb309.pdf)

### Checked escrowed dividends and lattice exercise

- Haug, Haug and Lewis describe the shifted stock volatility of escrow
  and the effect of changing the reserve with option expiry. Lu gives
  the stock and strike adjustments under a common reserve horizon.
  [Back to Basics](https://citeseerx.ist.psu.edu/document?doi=414dac84d6185d1840a20e1f38e1858004f5d4d1&repid=rep1&type=pdf),
  [Discrete dividends after maturity](https://arxiv.org/abs/2607.24973)
- QuantLib's escrowed finite-difference adjustment illustrates use
  of the discount-to-yield factor ratio in reserve carry.
  [QuantLib escrow adjustment](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/methods/finitedifferences/utilities/escroweddividendadjustment.cpp#L34-L41)
- Vellekoop and Nieuwenhuis separate pre- and post-dividend value
  and use interpolation across a piecewise lognormal stock jump.
  Their model gives an alternative for comparison. The exact-node
  exercise and delayed-payment claim conventions are specific to
  [record 002](decisions/financial/models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise).
  [Efficient Pricing of Derivatives on Assets with Discrete Dividends](https://ris.utwente.nl/ws/portalfiles/portal/6787112/Vellekoop06efficient.pdf)

### Discrete-dividend jumps and modeled ex-values

- Haug, Haug and Lewis distinguish the requested dividend from its
  modeled payout when equity is insufficient. Their liquidator policy
  motivates the capped stock drop and absorbing zero selected in the
  accepted Monte Carlo contract.
  [Back to Basics](https://citeseerx.ist.psu.edu/document?doi=414dac84d6185d1840a20e1f38e1858004f5d4d1&repid=rep1&type=pdf).
- Vellekoop and Nieuwenhuis analyze stock diffusion between discrete
  dividend dates and the stock transition at the ex boundary.
  The accepted model selects exact grid placement and a post-ex
  observation phase for its own Monte Carlo driver.
  [Efficient Pricing of Derivatives on Assets with Discrete Dividends](https://ris.utwente.nl/ws/portalfiles/portal/6787112/Vellekoop06efficient.pdf).
- Buryak and Guo distinguish nominal and modeled cash in their
  analysis of forward and parity effects near depletion.
  [Put–call parity and discrete dividends](https://arxiv.org/abs/1407.7328).

### Checked tree discounting and rollback

- Ho and Lee use one-period discount bonds and backward
  substitution for term-structure contingent claims.
  [Term Structure Movements and Pricing Interest Rate Contingent Claims](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1986.tb02528.x)
- QuantLib's lattice implementation uses the same departing-node
  discount and branch weights for forward state prices and
  backward expectation. [Forward and backward lattice operators](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/methods/lattices/lattice.hpp)
- QuantLib's callable bond API uses a benchmark maturity zero
  rate to convert a conventional OAS to one continuous shift.
  [Record 003](decisions/financial/models-numerical/tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)
  selects a local nodewise periodic restatement under the accepted
  scalar conversion. External comparisons must match the quote
  basis and discount law. [Callable bond OAS conversion](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/experimental/callablebonds/callablebond.cpp#L197-L230)

### Checked Ho–Lee curve fitting

- Ho and Lee take the observed initial term structure as given.
  Grant and Vora develop an analytical implementation of the
  short-rate model. The selected finite binomial law, noncontinuous
  bracket and acceptance controls are specified in
  [record 004](decisions/financial/models-numerical/ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting).
  [Ho and Lee, 1986](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1986.tb02528.x),
  [Grant and Vora, 2003](https://www.sciencedirect.com/science/article/pii/S1044028303000036)

### Checked BDT curve fitting

- Black, Derman and Toy construct a short-rate tree from the initial
  rate and yield-volatility structures and use it for bond options.
  [A One-Factor Model of Interest Rates and Its Application to Treasury Bond Options](https://rpc.cfainstitute.org/research/financial-analysts-journal/1990/a-one-factor-model-of-interest-rates-and-its-application-to-treasury-bond-options)
- Boyle, Tan and Tian distinguish the conditional short-rate
  volatility calibration from the yield-volatility construction
  and analyze their feasibility. [Record 005](decisions/financial/models-numerical/bdt-lognormal-curve-fit.md#accepted-decision-for-checked-bdt-curve-fitting)
  selects a supplied constant log-rate volatility and one
  checked curve fit per outgoing row.
  [Calibrating the Black–Derman–Toy Model](https://www.tandfonline.com/doi/abs/10.1080/13504860110062049)

### Checked BK trinomial fitting

- Black and Karasinski present a lognormal short-rate model
  with deterministic time variation of the target, mean reversion
  and volatility. [Record 006](decisions/financial/models-numerical/black-karasinski-trinomial.md#accepted-decision-for-checked-bk-trinomial-fitting)
  selects constant mean reversion and volatility and fits the
  initial curve with log-rate row centers.
  [Bond and Option Pricing when Short Rates are Lognormal](https://rpc.cfainstitute.org/research/financial-analysts-journal/1991/bond-and-option-pricing-when-short-rates-are-lognormal)
- Hull and White describe the capped branching alternatives and
  match the first two conditional moments. The selected BK
  lattice uses the exact OU moment inputs in those branch formulas.
  [Numerical Procedures for Implementing Term Structure Models I](https://doi.org/10.3905/jod.1994.407902)
- Daniluk and Muchorski derive continuous-model approximations
  for bond and swaption prices. Their results provide a comparator
  candidate with matching inputs and separately qualified error.
  [Approximations of Bond and Swaption Prices in a Black–Karasiński Model](https://arxiv.org/abs/1506.00697)

### Checked Hull–White event lattices and node bonds

- Hull and White describe state-price calibration, exact conditional
  moments as an option in moment matching, and variable-step
  trinomial construction. [Record 007](decisions/financial/models-numerical/hull-white-event-grid-and-affine-bonds.md#accepted-decision-for-checked-hullwhite-event-lattices-and-node-bonds)
  selects exact moments, nearest-mean branches and a checked prepared
  grid for the additive Gaussian model.
  [Numerical Procedures for Implementing Term Structure Models I](https://doi.org/10.3905/jod.1994.407902),
  [Using Hull–White Interest Rate Trees](https://www-2.rotman.utoronto.ca/~hull/downloadablepublications/USETREE.pdf)
- The continuous conditional bond follows the accepted centered
  affine law. Its use for exercise with finite-lattice discounting
  requires separate grid and price qualification.
  [Hull–White analytic kernels](#checked-hullwhite-analytic-kernels-and-payment-measures),
  [Hull–White rate clock and term projection](#hullwhite-rate-clock-curve-fit-and-term-projection)

### Checked rates-credit marginal calibration

- Nelson and Ramaswamy study recombining binomial diffusion
  approximations. [Record 008](decisions/financial/models-numerical/rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration)
  specifies a local binary mean-matching law with checked variance
  retention and a separately fitted rate and hazard level.
  [Simple Binomial Processes as Diffusion Approximations in Financial Models](https://doi.org/10.1093/rfs/3.3.393)
- Lando and Duffie–Singleton give intensity-based credit valuation
  context. Survival derives from integrated intensity. Joint
  discount and survival payoffs depend on the joint law.
  The positive-part floor and finite-grid acceptance rules are
  specific to record 008.
  [On Cox Processes and Credit Risky Securities](https://researchprofiles.ku.dk/en/publications/on-cox-processes-and-credit-risky-securities/),
  [Modeling Term Structures of Defaultable Bonds](https://web.stanford.edu/~duffie/jstorlinks/ds2.pdf)

### Checked rates-credit coupling and path sampling

- Erdely gives the Fréchet interval and cell formulas for two
  Bernoulli variables. [Record 009](decisions/financial/models-numerical/rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling)
  applies the accepted strict law to the prepared innovations
  and checks the intersection over departing row extrema.
  [A Subcopula Based Dependence Measure, Example 3.1](https://www.kybernetika.cz/content/2017/2/231/paper.pdf)
- Salmon, Moraes, Dror and Shaw describe counter-based random
  numbers and Philox. The project profile selects the seed,
  path, word-pair and midpoint assignments. Random123 supplies
  independent block reference vectors.
  [Parallel Random Numbers](https://www.thesalmons.org/john/random123/papers/random123sc11.pdf),
  [Random123 known-answer vectors](https://raw.githubusercontent.com/DEShawResearch/random123/main/tests/kat_vectors)

### Checked joint credit rollback and coupon folding

- Lando's Cox-process valuation separates payments conditional on survival
  from default-time payments. Duffie's continuous recovery treatment covers
  fractional recovery of par. [Record 010](decisions/financial/models-numerical/rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding)
  derives its local recovery weight by holding the interval rate and
  intensity constant in the default-time integral.
  [Lando, Proposition 3.1](https://globalriskguard.com/resources/fideriv/lando_cox.pdf),
  [Duffie, §2.4](https://web.stanford.edu/~duffie/par.pdf)
- Bianchetti distinguishes discounting and index forwarding curves and
  discusses changes of numeraire. The accepted gross-ratio normalization
  is a project choice for the prepared one-factor rate tree and matches
  the first moment under its index-maturity forward measure. The record
  derives the coupon fold by conditioning on fixing and survival to payment.
  [Two Curves, One Price](https://arxiv.org/abs/0905.2770)

<a id="garman-kohlhagen-1983"></a>

### Garman Kohlhagen 1983

- Garman, M. B., and Kohlhagen, S. W. "Foreign Currency Option Values." FX
  option formula recovered by treating the foreign rate as a continuous yield.

### FX delta smile construction and materialization

- Reiswich and Wystup derive premium-unadjusted forward delta and its strike
  inversion in sections 1.3–1.5. Table 3 gives the delta-neutral straddle
  strike. Equations 31–32 define the arithmetic smile-strangle measure and
  reconstruct wing quotes. Their market-strangle section gives the distinct
  premium constraint. [Reiswich and Wystup working paper](https://www.mathfinance.com/wp-content/uploads/2025/04/FXVolatility-Smile-Construction_CPQF_Arbeits20_neu2.pdf)
- The quote profile is governed by
  [CORE-MARKET-15](decisions/financial/core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs).
  The selected direct interpolation, strict expiry boundary and exact-union
  materialization are local model choices.
  [Accepted record 17](decisions/financial/models-analytic/fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization)

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

<a id="normalized-jump-characteristic-laws"></a>

### Forward-normalized characteristic laws for COS

- Merton gives the independent compound-Poisson model and expected relative
  jump compensator in equations 2–3. Equations 16–19 give the conditional
  mixture used for pricing.
  The selected pricing inputs describe normal log jumps under the cash
  measure. [Original paper](https://pages.stern.nyu.edu/~dbackus/Disasters/Merton%20jumps%20JFE%2076.pdf)
- Madan, Carr and Chang give the Gamma-time mixture and the independent
  Gamma difference in equations 5–8. Equation 22 gives the risk-neutral
  compensator. The discussion after equation 25 relates the option formula
  to a stock-weighted probability measure.
  [Original paper](https://engineering.nyu.edu/sites/default/files/2018-09/CarrEuropeanFinReview1998.pdf)
- The selected forward normalization, reflected share parameters and stable
  centered formulas follow by applying the exponential tilt and subtracting
  the selected mean. The sign, range and error contracts are specified in
  the [accepted record 11](decisions/financial/models-analytic/jump-characteristic-laws.md#accepted-decision-for-checked-characteristic-laws).

<a id="fang-oosterlee-2008"></a>

### Fang Oosterlee 2008

- Fang, F., and Oosterlee, C. W. "A Novel Pricing Method for European Options
  Based on Fourier-Cosine Series Expansions." COS method used by the Fourier
  option primitives.

<a id="cos-payoff-window-and-series"></a>

### COS payoff window and series

- Fang and Oosterlee derive the half-weighted expansion and analytic payoff
  coefficients in equations 22–30. Section 4 distinguishes finite-window
  and finite-series effects. Equation 49 gives the cumulant window heuristic,
  and Remark 5.2 explains the bounded put payoff choice.
  [Original paper](https://ir.cwi.nl/pub/13283/13283D.pdf)
- Junike and Pankrashkin show that a narrow cumulant window can lead to a
  materially wrong COS price even as the number of terms increases. See the
  Merton example in Section 4.3.
  [Precise option pricing by the COS method](https://arxiv.org/abs/2109.01030)
- Joshi studies COS under a change of numeraire. The selected cash/share
  payoff coordinates, reflection error and bounded term selection are
  specified in the [accepted record 10](decisions/financial/models-analytic/cos-inversion.md#accepted-decision-for-checked-cos-pricing).
  [Comparison of various variants of the COS method](https://doi.org/10.1016/j.orl.2017.01.006)

<a id="andersen-2008-heston-qe"></a>

### Andersen 2008 Heston QE

- Andersen, L. "Simple and Efficient Simulation of the Heston Stochastic
  Volatility Model." Quadratic-exponential discretization used by the Heston
  Monte Carlo pricers. Sections 3.2 and 4 describe the moment-matched
  transition and the use of its exponential moments.
  [Author's paper](https://www.maths.univ-evry.fr/pages_perso/crepey/Finance/andersen%20vol%20sto.pdf).

### Checked Heston QE-M spot paths

- Andersen's sections 3.2 and 4.3 give the QE variance law and
  exponential-moment spot correction. Section 5 uses trapezoidal
  weights. The ideal conditional forward property applies on the
  domain of the selected QE law's moment, including the mass at
  zero in the mixture. Record 09 selects a run failure at a moment
  pole and evaluates the correction in centered form.
  [Andersen's paper](https://www.maths.univ-evry.fr/pages_perso/crepey/Finance/andersen%20vol%20sto.pdf).
- Broadie and Kaya sample the integrated variance conditional on
  the variance endpoints in their exact Heston method. It provides
  an independent reference for grid-bias comparisons with QE-M's
  endpoint-based integral. The deterministic zero-vol-of-vol limit
  integrates the mean-reverting variance directly.
  [Broadie and Kaya's paper](https://www.columbia.edu/~mnb2/broadie/Assets/broadie_kaya_exact_sim_or_2006.pdf).

### Revolving-credit utilization and joint paths

- Gillespie gives the exact Gaussian transition of a scalar
  Ornstein–Uhlenbeck process and its integral. The utilization
  model uses a frozen-target endpoint followed by saturation,
  which changes its moments and creates boundary mass.
  [Exact OU simulation](https://doi.org/10.1103/PhysRevE.54.2084).
- Hull and White develop the extended Gaussian rate model.
  Andersen supplies the QE law used for the credit spread.
  The proposed composition correlates latent step normals and
  preserves the Gaussian endpoint/integral covariance for the
  fitted rate, with a separately specified cross-factor coupling.
  [Hull and White](https://doi.org/10.1093/rfs/3.4.573),
  [Andersen](https://www.maths.univ-evry.fr/pages_perso/crepey/Finance/andersen%20vol%20sto.pdf).
- Jiménez, Lopez and Saurina find that borrower distress and
  supply conditions affect corporate line utilization. The
  empirical work motivates testing a link and supplies no
  coefficient or pricing-measure fit for the selected model.
  [Empirical Analysis of Corporate Credit Lines](https://www.bis.org/bcbs/events/rtf07lopez.pdf).

### Checked CIR QE and deterministic shifts

- Cox, Ingersoll and Ross derive the square-root process and its
  conditional moments and transition law. Malham and Wiese develop
  exact transition sampling. The accepted QE law uses the exact
  first two conditional moments and has its own distribution and
  atom at zero.
  [A Theory of the Term Structure of Interest Rates](https://pages.stern.nyu.edu/~dbackus/BCZ/discrete_time/CIR_Econometrica_85.pdf),
  [Chi-square simulation of the CIR process and the Heston model](https://arxiv.org/abs/0802.4411).
- Brigo and Mercurio separate the unshifted factor from the
  deterministic short-rate shift. A curve fit uses a qualified shift
  integral. The accepted raw route uses the supplied shift and
  publishes the checked short-rate endpoint.
  [On deterministic-shift extensions of short-rate models](https://www.ma.imperial.ac.uk/~dbrigo/detshiftrep.pdf).

<a id="reiner-rubinstein-1991"></a>

### Reiner Rubinstein 1991

- Reiner, E., and Rubinstein, M. *Breaking Down the Barriers*, Risk, 1991.
  Equations [1]–[4] describe the direct and reflected payoff terms.
  Equations [5]–[6] distinguish expiry survival cash and discounted
  first-passage cash. [Original article](https://www.academia.edu/16585738/Breaking_down_the_barriers)
- QuantLib's analytic barrier engine provides a second implementation of the
  strike regimes and real-root at-hit formula. The finite-horizon treatment
  of negative discriminants is a separate numerical choice in
  [MODELS-ANALYTIC-05](decisions/financial/models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash).
  [QuantLib source](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/barrier/analyticbarrierengine.cpp)

### Sampled log barrier crossing and first passage

- Shevchenko, sections 2.3 and 2.4, derives single-barrier
  crossing probabilities conditional on simulated endpoints and
  states the cellwise drift and volatility assumptions.
  [Brownian-bridge barrier simulation](https://arxiv.org/pdf/0904.1157).
- Casella and Roberts, appendix A, derive the transformed conditional
  first-passage law of a Brownian bridge. The barrier package uses
  the crossing uniform to select the conditional time and qualifies
  the discount law independently.
  [Conditional first-passage derivation](https://wrap.warwick.ac.uk/id/eprint/35492/1/WRAP_Casella_08-19w.pdf).
- Broadie, Glasserman and Kou derive an asymptotic barrier shift for
  approximating discretely monitored contracts with continuous pricing.
  The sampled continuous bridge uses the contractual level.
  [Continuity correction](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

<a id="kemna-vorst-1990"></a>

### Kemna Vorst 1990

- Kemna, A. G. Z., and Vorst, A. C. F.
  [A Pricing Method for Options Based on Average Asset Values](https://doi.org/10.1016/0378-4266(90)90039-5).
  Section 4 and Appendix B derive the geometric average's lognormal law.
  The paper's discrete grid includes time zero. Apply the weights for the
  actual grid. See also [QuantLib's discrete geometric engine](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/asian/analytic_discr_geom_av_price.cpp).

### Discrete Asian payoff and settlement

- Boyle, Broadie and Glasserman, section 2.6, give the discrete arithmetic
  Asian payoff with the count-normalized average and illustrate a geometric
  control variate. Kemna and Vorst derive the geometric lognormal law under
  positive GBM. The same-path control expectation uses the actual fixing
  law, including history and tied times.
  [Monte Carlo methods for security pricing](https://business.columbia.edu/sites/default/files-efs/pubfiles/4336/monte_carlo_methods_security_pricing.pdf),
  [Kemna–Vorst](https://doi.org/10.1016/0378-4266(90)90039-5).
- FpML's equity options documentation distinguishes averaging dates
  from the option's delivery or cash settlement. The payoff record
  specifies its own exact node mapping, scale and observation count.
  [Equity derivative options product architecture](https://www.fpml.org/spec/fpml-5-3-6-rec-1/html/confirmation/fpml-5-3-intro-9.html).

<a id="turnbull-wakeman-1991"></a>

### Turnbull Wakeman 1991

- Turnbull, S. M., and Wakeman, L. M.
  [A Quick Algorithm for Pricing European Average Options](https://doi.org/10.2307/2331213).
  Section II uses a two-moment lognormal baseline and higher-cumulant
  Edgeworth terms. [MODELS-ANALYTIC-04](decisions/financial/models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing)
  selects the baseline for the future contribution, with the known
  contribution treated as a payoff shift. [QuantLib's moment engine](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/asian/turnbullwakemanasianengine.cpp)
  provides a comparison for the moment and effective-strike formulas.

<a id="conze-viswanathan-1991"></a>

### Conze Viswanathan 1991

- Conze, A., and Viswanathan, R. *Path Dependent Options: The Case of Lookback
  Options*. Historical reference for fixed-strike lookbacks. [Publisher record](https://doi.org/10.1111/j.1540-6261.1991.tb04648.x)
- QuantLib's fixed engine separates locked payoff from the remaining maximum
  or minimum premium. Its Gaussian terms and analytic zero-carry derivative
  provide formula comparisons for [MODELS-ANALYTIC-06](decisions/financial/models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums).
  The record selects its own carry and numerical admission.
  [QuantLib fixed lookback source](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/lookback/analyticcontinuousfixedlookback.cpp)

<a id="goldman-sosin-gatto-1979"></a>

### Goldman Sosin Gatto 1979

- Goldman, M. B., Sosin, H. B., and Gatto, M. A. "Path Dependent Options: Buy
  at the Low, Sell at the High." *Journal of Finance*, 34(5), 1111-1127.
  Historical reference for continuous floating-strike options. [Publisher record](https://doi.org/10.1111/j.1540-6261.1979.tb00059.x)
- QuantLib's floating engine supplies the signed formulas and zero-carry
  derivative. [QuantLib floating lookback source](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/lookback/analyticcontinuousfloatinglookback.cpp)
- Broadie, Glasserman and Kou, *Connecting Discrete and Continuous
  Path-Dependent Options*, Section 2, give the lookback payoff definitions and
  the relationship between the fixed-call and floating-put payoffs.
  [Original paper](https://business.columbia.edu/sites/default/files-efs/pubfiles/1851/connecting.pdf)

### Sampled lognormal lookback extrema

- Conze and Viswanathan treat fixed-strike lookback payoffs. Goldman,
  Sosin and Gatto treat floating exercise levels defined by the
  path extremum.
  [Fixed-strike lookbacks](https://doi.org/10.1111/j.1540-6261.1991.tb04648.x),
  [Floating-strike lookbacks](https://doi.org/10.1111/j.1540-6261.1979.tb00059.x).
- Shevchenko, section 2.3, gives the conditional one-dimensional
  Brownian-bridge extremum laws by reflection. Inverting one
  tail gives the sampled extremum used for one payoff direction.
  Beaglehole, Dybvig and Zhou discuss the simulation bias caused
  by observing continuous extrema only at nodes.
  [Conditional extrema](https://arxiv.org/pdf/0904.1157),
  [Simulation of path extrema](https://doi.org/10.2469/faj.v53.n1.2057).
- Broadie, Glasserman and Kou relate continuous and discrete
  extrema prices by asymptotic corrections. The exact-node
  payoff and sampled continuous bridge each use their selected
  monitoring law directly.
  [Connecting discrete and continuous path-dependent options](https://www.columbia.edu/~sk75/f%26sBGK.pdf).

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

### Checked variance replication and explicit Black wings

- Demeterfi, Derman, Kamal and Zou derive the weighted put and call
  strip and log payoff and discuss finite strike coverage and jumps
  in section V. [More Than You Ever Wanted to Know About Volatility Swaps](https://emanuelderman.com/wp-content/uploads/1999/02/gs-volatility_swaps.pdf)
- Cboe defines its quoted-strike selection and adjustment for its index.
  The forward split, mesh comparison and positive integral of Black vega
  for the selected full wings are model choices in
  [accepted record 21](decisions/financial/models-analytic/variance-replication.md#accepted-decision-for-checked-variance-replication-and-explicit-black-wings).
  [Cboe VIX methodology](https://cdn.cboe.com/resources/vix/VIX_Methodology.pdf)

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

### Checked price-equivalent ATM volatility conversion

- Schachermayer and Teichmann give the exact ATM Normal price and
  compare the Normal and Black pricing laws in section 2.1.
  Choi et al. derive analytic at-the-forward volatility conversion in
  section 4.2, equations 13–15.
  [Bachelier versus Black–Scholes](https://arxiv.org/pdf/0711.1272),
  [A Black–Scholes user's guide to the Bachelier model](https://arxiv.org/pdf/2104.08686)
- NIST gives the inverse-error-function identities and expansions.
  The selected domain, checked analytic inversion and two replay
  controls are in
  [accepted record 22](decisions/financial/models-analytic/atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion).
  [NIST DLMF, section 7.17](https://dlmf.nist.gov/7.17)

<a id="implied-volatility-inversion-and-conditioning"></a>

### Implied volatility inversion and conditioning

- [QuantLib's official Black/normal implementation](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/pricingengines/blackformula.cpp),
  inspected2026-11-09: Black inversion uses OTM parity and returns total standard
  deviation; compare annual Black sigma after division by sqrt(T). The Bachelier
  inverse returns annual normal volatility in underlying units. This supports
  coordinate/normalization comparisons, not adoption of its iteration family,
  displacement restrictions, approximation shortcuts or numerical bounds.
- [Strata's official Black implementation](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/impl/option/BlackFormulaRepository.java),
  inspected2026-11-09: the regular interior inverse-price adjoint is inverse vega. This supports
  the low-vega conditioning qualification; it does not establish uniform IV
  digits or validate this project's Brent/log-OTM acceptance profile.
  The [accepted 03 decision](decisions/financial/models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance)
  separates strict model bounds, operational controls and paired premium gates.

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

### Raw SVI admission, density and calibration

- Gatheral and Jacquier give the raw parameterization in section 3.1 and
  the density-sign factor and vanishing right-call condition in equation
  2.1 and Lemma 2.2. The continuous condition concerns all log-moneyness.
  [Arbitrage-free SVI volatility surfaces](https://arxiv.org/pdf/1204.0646)
- Lee derives the moment relations and weak slope ceilings. Martini and
  Mingone give the strict right-wing boundary for a vanishing raw-SVI call
  in Lemma 2.1 and discuss the left-wing zero-atom boundary in section 2.
  [Lee's moment formula](https://math.uchicago.edu/~rogerlee/moment.pdf),
  [No arbitrage SVI](https://arxiv.org/pdf/2005.03340)
- The checked solver, quote-replay budget and finite density gate are local
  fit choices. [Accepted record 18](decisions/financial/models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance)

<a id="hagan-2002-sabr"></a>

### Hagan 2002 SABR

- Hagan, P. S., Kumar, D., Lesniewski, A., and Woodward, D. "Managing Smile
  Risk." Canonical SABR reference.

<a id="sabr-cev-distance-and-time-corrections"></a>

### SABR CEV distance and time corrections

- Hagan, Kumar, Lesniewski and Woodward give the geometric-midpoint time
  factors for Normal and Black in Appendix B, including equations B.69a
  and B.69c. The selected profile retains these time factors.
  [Managing Smile Risk](https://derivativesacademy.com/storage/uploads/files/modules/resources/1702213496_hagan_kumar_lesniewski_woodward_managing_smile_risk.pdf)
- Obłój derives the corrected Black leading term in Table 1 and equation 3
  and considers it with the Hagan time correction. The exact CEV distance
  preserves the beta-to-one limit.
  [Fine-tune your smile](https://arxiv.org/pdf/0708.0998)
- Hagan, Lesniewski and Woodward give the corresponding normal leading
  coordinate in Section 5.2. The shared skew identity, signed shift
  admission and selected numerical limits are specified in the
  [accepted record 13](decisions/financial/models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes).
  [Probability Distribution in the SABR Model](https://lesniewski.us/papers/working/ProbDistrForSABR.pdf)

<a id="sabr-atm-pin-and-fit-policy"></a>

### SABR ATM conditioning and fit policy

- West uses a fixed-beta SABR fit conditioned on ATM volatility. Sections
  7–8, including equations 20–21, derive the Black ATM alpha equation and
  discuss root selection. [Calibration of the SABR Model in Illiquid Markets](https://doi.org/10.1080/13504860500148672)
- The [accepted record 14](decisions/financial/models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration)
  applies the accepted record 13 Normal or Black quote and checks the
  selected bracket for a unique increasing root. The weights, work,
  start ordering and native quote gate are project decisions. The
  [accepted LM](decisions/financial/core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
  and [accepted Brent](decisions/financial/core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
  records own the numerical kernels.

<a id="sabr-delta-and-finite-strike-grid"></a>

### SABR ATM delta and finite-strike call grids

- Reiswich and Wystup give the premium-unadjusted forward delta in
  Section 2, equation 16. Hagan and Lesniewski derive SABR hedge delta
  conventions with smile sensitivities. The selected ATM proxy fixes
  one volatility when solving the strike.
  [FX volatility smile construction](https://www.deriscope.com/docs/FX_volatility_smile_construction_Reiswich_Wystup_2009.pdf),
  [Bartlett's delta in the SABR model](https://arxiv.org/pdf/1704.03110)
- Carr and Madan derive the bounded vertical payoff and unequal-strike
  butterfly in Section 2. Davis and Hobson's Theorem 3.1 includes the
  upper-strike conditions required for full single-expiry consistency.
  [A note on sufficient conditions for no arbitrage](https://engineering.nyu.edu/sites/default/files/2018-09/CarrFinResearchLetters2005.pdf),
  [The Range of Traded Option Prices](https://warwick.ac.uk/fac/sci/statistics/staff/academic-research/hobson/publications/range.pdf)
- Cohen, Reisinger and Wang develop local discrete conditions and
  constrained repair. The finite-grid objective, exact down-only
  construction and repricing gates selected here are in the
  [accepted record 15](decisions/financial/models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair).
  [Detecting and repairing arbitrage in traded option prices](https://arxiv.org/pdf/2008.09454)

### Volatility-source interpolation and materialization

- Gatheral and Jacquier's Lemma 2.1 states the calendar criterion under
  proportional dividends in terms of total variance at fixed forward
  log-moneyness. The selected fixed-absolute-strike blend uses a different
  coordinate and does not confer that guarantee.
  [Arbitrage-free SVI volatility surfaces](https://arxiv.org/pdf/1204.0646)
- The SABR expansion and its units follow
  [the accepted quote kernel](decisions/financial/models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes).
  The distinct interpolation, materialization and quote-comparison choices are
  in [accepted record 16](decisions/financial/models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).
  [Hagan et al., Managing Smile Risk](#hagan-2002-sabr)

<a id="hagan-2003-cms-convexity"></a>

### Hagan 2003 CMS Convexity

- Hagan, P. S. "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
  *Wilmott Magazine*, March 2003. CMS convexity adjustment under the annuity
  measure.

<a id="bayer-friz-gatheral-2016"></a>

### Bayer Friz Gatheral 2016

- Bayer, C., Friz, P., and Gatheral, J. "Pricing under rough volatility."
  *Quantitative Finance*, 16(6), 887-904. Rough Bergomi and forward-variance
  curve inputs. [Original paper](https://doi.org/10.1080/14697688.2015.1099717).

### Rough Bergomi adapted steps and moment limits

- Bayer, Friz and Gatheral give the conditional forward-variance
  model and its shared Brownian driver. McCrickerd and Pakkanen
  describe a hybrid-grid simulation and condition on the common
  driver to evaluate options. The proposed left-point step uses
  sampled-grid variance for Gaussian normalization.
  [Original model](https://www.realvol.com/RVOL2Paper.pdf),
  [Simulation and conditioning](https://arxiv.org/pdf/1708.02563).
- Gassiat distinguishes the continuous model's true and strict
  local martingale regimes and proves higher-moment divergence
  under stated conditions. The proposed grid's first-moment
  identity and direct call variance follow separately by
  conditioning on the current shock and the previous cell's
  independent RL residual. The European estimator uses grid
  put–call parity and bounded put observations.
  [Continuum martingale and moment results](https://arxiv.org/pdf/1811.10935).

<a id="gatheral-jaisson-rosenbaum-2018"></a>

### Gatheral Jaisson Rosenbaum 2018

- Gatheral, J., Jaisson, T., and Rosenbaum, M. "Volatility is rough."
  *Quantitative Finance*, 18(6), 933-949. Empirical roughness of volatility
  and the Hurst exponent near 0.1.

<a id="el-euch-rosenbaum-2019"></a>

### El Euch Rosenbaum 2019

- El Euch, O., and Rosenbaum, M. "The characteristic function of rough Heston
  models." *Mathematical Finance*, 29(1), 3-38. Rough Heston dynamics and
  characteristic function. [Original paper](https://arxiv.org/pdf/1609.02108).

### Rough Heston hybrid paths and projection

- El Euch and Rosenbaum give the fractional variance equation.
  Abi Jaber, Larsson and Pulido cover the fractional case in
  Example 7.2 and prove the continuous stock martingale in
  Lemma 7.3. The proposed grid uses exact newest-cell Gaussian
  covariance and an older-cell projection onto Brownian increments.
  [Fractional Heston law](https://arxiv.org/pdf/1609.02108),
  [Affine Volterra Processes](https://arxiv.org/pdf/1708.08796).
- Bennedsen, Lunde and Pakkanen describe exact near-cell integrals
  and average-kernel projection. Richard, Tan and Yang analyze
  related rough Heston Euler schemes using positive parts in their
  coefficients. Convergence of the proposed projected hybrid
  requires its own evidence.
  [Hybrid cell projection](https://pure.au.dk/ws/files/135819738/Bennedsen2017_Article_HybridSchemeForBrownianSemista.pdf),
  [Related discrete-time simulation schemes](https://arxiv.org/pdf/2107.07835).
- Gerhold, Gerstenecker and Pinter analyze continuum moment
  explosions. The finite-grid Gaussian power-moment bound in
  record 15 follows from its square-root history recursion.
  Bayer and Brêneis provide a separate weak simulation approach
  for benchmark comparisons.
  [Continuum moment explosions](https://arxiv.org/pdf/1801.09458),
  [Weak simulation benchmark](https://arxiv.org/abs/2310.04146).

<a id="rough-heston-lewis-and-tail-comparison"></a>

### Rough Heston Lewis inversion and tail comparison

- El Euch and Rosenbaum derive the fractional Riccati equation and
  fractional-integral coefficient and discuss product integration in
  Section 5. The selected guarded fully implicit root is a numerical
  decision in record 12. [Original paper](https://arxiv.org/pdf/1609.02108)
- Abi Jaber, Larsson and Pulido give the affine Volterra moment strip,
  stock martingale and real-part sign in Theorem 7.1, Lemmas 7.3–7.4 and
  Example 7.2. Theorem C.2 supports the positive linear comparison in
  the tail derivation. [Affine Volterra processes](https://arxiv.org/pdf/1708.08796)
- Lewis uses the payoff transform and half contour in Section 3 and
  Appendix B. The normalized minimum-payoff identity applies directly
  to the admitted terminal law. [Original paper](https://www.maths.univ-evry.fr/pages_perso/crepey/Finance/ExpLevy.pdf)
- Gerhold, Gerstenecker and Pinter give the scalar equilibrium comparison
  in Lemma 2 and Proposition 5. Simon's Theorem 4, equation 6.8, bounds
  the Mittag–Leffler relaxation function. The specific coarse attenuation
  and dyadic cutoff enclosure follow by applying these to the half
  contour in the [accepted record 12](decisions/financial/models-analytic/rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing).
  [Moment explosions in the rough Heston model](https://arxiv.org/abs/1801.09458),
  [Comparing Fréchet and positive stable laws](https://arxiv.org/pdf/1310.1888)

<a id="bennedsen-lunde-pakkanen-2017"></a>

### Bennedsen Lunde Pakkanen 2017

- Bennedsen, M., Lunde, A., and Pakkanen, M. S. "Hybrid scheme for Brownian
  semistationary processes." *Finance and Stochastics*, 21(4), 931-965.
  Hybrid simulation of Volterra / rough-volatility kernels.

<a id="mccrickerd-pakkanen-2018"></a>

### McCrickerd Pakkanen 2018

- McCrickerd, R., and Pakkanen, M. S. "Turbocharging Monte Carlo pricing for
  the rough Bergomi model." *Quantitative Finance*, 18(11), 1877-1886.

### Fractional noise and actual-grid Volterra projection

- Mandelbrot and Van Ness define fractional Brownian motion
  with stationary increments. The standalone sampler uses the
  accepted unit-normalized increment covariance with a
  chronological lower factor.
  [Original fBM paper](https://epubs.siam.org/doi/10.1137/1010093).
- Bennedsen, Lunde and Pakkanen use power-kernel integrals near
  the endpoint and a step approximation elsewhere. Proposition
  2.8 derives the cell-average constant as an orthogonal
  projection. The proposal applies that projection to each
  actual cell and retains the exact newest integral. The
  projection preserves covariance with its Brownian increment
  while changing the variance and covariance of the sampled
  fractional levels.
  [Hybrid scheme](https://pure.au.dk/ws/files/135819738/Bennedsen2017_Article_HybridSchemeForBrownianSemista.pdf).
- McCrickerd and Pakkanen use the shared Brownian driver in
  rough Bergomi leverage. For the proposed grid law, normalizing
  a Gaussian exponential uses the variance of the sampled
  level. Product convergence requires separate evidence.
  [Rough Bergomi simulation](https://arxiv.org/abs/1708.02563).

### Cheyette state and rough volatility extension

- Cheyette derives the separable HJM state reduction in equations 4–7.
  García-Lorite and Merino give the corresponding short rate and
  two state equations in section 3.2, along with the bond formula
  under an initial forward curve.
  [Cheyette's Interest Rate Models](https://catalogimages.wiley.com/images/db/pdf/E0471220949.01.pdf),
  [Convexity adjustments à la Malliavin](https://arxiv.org/html/2304.13402#S3.SS2).
- Gillespie gives the exact Gaussian OU transition. Bennedsen, Lunde
  and Pakkanen give the hybrid power-kernel construction. The
  [accepted rough Cheyette record](decisions/financial/models-monte-carlo/16-rough-cheyette.md#accepted-decision-for-checked-rough-cheyette-paths)
  combines these with left-frozen amplitude and a projection that
  preserves rate covariance with the actual Brownian increment.
  Its within-cell rough covariance and financial qualification are
  separate from those references.
  [Exact OU simulation](https://doi.org/10.1103/PhysRevE.54.2084),
  [Hybrid scheme](https://arxiv.org/abs/1507.03004).

<a id="schwartz-smith-2000"></a>

### Schwartz Smith 2000

- Schwartz, E., and Smith, J. E. "Short-Term Variations and Long-Term Dynamics
  in Commodity Prices." *Management Science*, 46(7), 893-911. Two-factor
  commodity spot model. [Authors' paper](https://www.anderson.ucla.edu/documents/areas/fac/finance/72.pdf).

### Schwartz Smith joint transition and spot expectation

- Schwartz and Smith give the OU and arithmetic Brownian factor
  means and covariance in sections 2 and 3. Their pricing law
  applies constant drift adjustments and equation 9 gives the
  risk-neutral expectation of future spot. The proposed step
  uses the integrated Gaussian covariance, with scaled loadings
  for its conditional residual and log-spot variance.
  [Schwartz and Smith](https://www.anderson.ucla.edu/documents/areas/fac/finance/72.pdf).
- Section 3.1 relates the risk-neutral expectation to futures
  and states the conditions for forward equality. Cox, Ingersoll
  and Ross analyze the forward and futures distinction under
  stochastic rates. Curve calibration and a one-expiry terminal
  mean constraint serve different purposes.
  [Schwartz and Smith](https://www.anderson.ucla.edu/documents/areas/fac/finance/72.pdf),
  [Cox, Ingersoll and Ross](https://spinup-000d1a-wp-offload-media.s3.amazonaws.com/faculty/wp-content/uploads/sites/34/2019/06/1981_ForwardFuturesPrices.pdf).

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

<a id="convertible-delta-bands"></a>

### Convertible Delta Bands

- Putnam Investments. Literature TL913: busted convertibles usually trade at 0 to
  40 delta and balanced convertibles at 40 to 80 delta.
  <https://www-dev.putnam.com/literature/pdf/TL913_IN.pdf>
- Lord Abbett. "The Road Ahead for the Convertible Bond Market." Credit-sensitive
  below 40% delta and equity-sensitive above 80%.
  <https://lordabbett.com/en-us/financial-advisor/insights/markets-and-economy/the-road-ahead-for-the-convertible-bond-market.html>
- MSCI. "Investing in Convertible Bonds." A different convention: bond-like below
  33% delta, balanced to 66% and equity-like above.
  <https://support.msci.com/www/blog-posts/investing-in-convertible-bonds/01180853317>

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

<a id="heston-transform-branch-and-moments"></a>

### Heston transform branch and moments

- Heston's two option-probability transforms and affine coefficients are
  given in equations 10 and 17 and the appendix. [Original paper](https://finance.martinsewell.com/stylized-facts/volatility/Heston1993.pdf)
- Albrecher, Mayer, Schoutens and Tistaert describe the decaying formulation.
  Lord and Kahl's formulation 2 and Theorem 3 establish the principal-log
  choice for the admissible transform. The branch result supplies no
  floating-point accuracy guarantee. [Little Heston Trap](https://www.ma.imperial.ac.uk/~ajacquie/IC_Num_Methods/IC_Num_Methods_Docs/Literature/HestonTrap.pdf) ·
  [Lord–Kahl](https://www2.math.uni-wuppertal.de/~kahl/publications/complexlogarithmsheston.pdf)
- Andersen and Piterbarg give the CIR accessibility condition and
  maturity-specific moment explosion. Keller–Ressel, Corollary 2.7 and
  Section 6.1, give the finite-horizon martingale criterion. These
  distinguish the share contour from arbitrary higher moments.
  [Andersen–Piterbarg](https://doi.org/10.1007/s00780-006-0011-7) ·
  [Keller–Ressel](https://arxiv.org/pdf/0802.1823)
- The selected real-frequency contours and numerical domain are in
  [MODELS-ANALYTIC-08](decisions/financial/models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform).

<a id="heston-fourier-price-and-tail"></a>

### Heston Fourier price and tail

- Heston, equations 10 and 18, gives the two-probability price and
  the upper-probability inversion. [Original paper](https://finance.martinsewell.com/stylized-facts/volatility/Heston1993.pdf)
- Kahl and Jäckel, Propositions 3.1–3.3, derive the large-frequency
  decay slope and removable origin. The slope is asymptotic.
  [Not-so-complex logarithms in the Heston model](https://www.maths.univ-evry.fr/pages_perso/crepey/Equities/heston%20log.pdf)
- Broadie and Kaya, Section 3.3, give the conditional Gaussian
  decomposition with orthogonal variance (1−rho²) times integrated
  variance. The square-root affine Laplace equation uses the same
  Riccati structure as Cox, Ingersoll and Ross, equation 23.
  The rational lower envelope for the decay exponent and conservative
  dyadic truncation bound are derived in the decision.
  [Broadie–Kaya](https://www.columbia.edu/~mnb2/broadie/Assets/broadie_kaya_exact_sim_or_2006.pdf) ·
  [CIR](https://pages.stern.nyu.edu/~dbackus/BCZ/discrete_time/CIR_Econometrica_85.pdf) ·
  [MODELS-ANALYTIC-09](decisions/financial/models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)

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

### Checked lognormal and local-volatility paths

- Black and Scholes give the lognormal pricing model. Glasserman covers
  exact lognormal path generation on a simulation grid. The accepted
  model also allows deterministic integrated carry and piecewise variance.
  [Black and Scholes](https://www.journals.uchicago.edu/doi/10.1086/260062),
  [Generating sample paths](https://link.springer.com/chapter/10.1007/978-0-387-21617-1_3).
- Dupire explains recovering a deterministic volatility term structure
  from ATM total variance and extending to state-dependent volatility.
  Gatheral and Jacquier give the calendar condition at every fixed log
  forward moneyness. The accepted ATM bootstrap samples only zero
  log forward moneyness at its requested expiries.
  [Pricing with a Smile](https://www.risk.net/sites/default/files/import_unmanaged/risk.net/data/risk/pdf/technical/2007/risk20_0707_technical_volatility.pdf),
  [Arbitrage-free SVI volatility surfaces](https://arxiv.org/abs/1204.0646).
- Glasserman treats discretized diffusion paths. The accepted local-volatility
  model uses the accepted instantaneous relative coefficient and freezes it
  at the left knot in log space. Product convergence depends on the grid
  and payoff. [Discretization methods](https://link.springer.com/chapter/10.1007/978-0-387-21617-1_6).

### Coordinatewise Euler and exact Gaussian transitions

- Higham presents Euler–Maruyama, Milstein and the assumptions behind
  their convergence and stability analysis. The accepted generic
  Euler operation selects a checked coordinatewise coefficient domain.
  [An Algorithmic Introduction to Numerical Simulation of Stochastic Differential Equations](https://epubs.siam.org/doi/10.1137/S0036144500378302).
- Gillespie derives the exact scalar OU transition. Särkkä and Solin
  give the conditional mean and covariance for linear SDEs in
  sections 6.1–6.2. The accepted multi-OU law specializes that
  covariance integral to diagonal drift with correlated Brownian
  drivers, then factors the finite-step innovation covariance.
  [Exact numerical simulation of the Ornstein–Uhlenbeck process and its integral](https://journals.aps.org/pre/abstract/10.1103/PhysRevE.54.2084),
  [Applied Stochastic Differential Equations](https://users.aalto.fi/~asolin/sde-book/sde-book.pdf).

### Checked Dupire extraction and local-volatility grids

- Gatheral derives local variance from total implied variance under deterministic
  proportional carry, with the time derivative at fixed log-forward moneyness.
  See chapter 1, equation 1.10. [The Volatility Surface](https://catalogimages.wiley.com/images/db/pdf/9780471792512.excerpt.pdf)
- Fornberg derives interpolation-based finite differences on arbitrary grids.
  The quadratic stencils, endpoint time secants and extraction boundaries
  are the local choices in
  [accepted record 19](decisions/financial/models-analytic/dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids).
  [Generation of Finite Difference Formulas on Arbitrarily Spaced Grids](https://doi.org/10.1090/S0025-5718-1988-0935077-0)

### Checked surface-arbitrage diagnostics and coverage

- Carr and Madan derive vertical and unequal-strike butterfly spread
  inequalities. Davis and Hobson's Theorem 3.1 shows the additional role
  of terminal-wing conditions for model existence.
  [Carr and Madan](https://engineering.nyu.edu/sites/default/files/2018-09/CarrFinResearchLetters2005.pdf),
  [Davis and Hobson](https://warwick.ac.uk/fac/sci/statistics/staff/academic-research/hobson/publications/range.pdf)
- Gatheral and Jacquier's Lemma 2.1 uses fixed forward log-moneyness
  under proportional dividends and Lemma 2.2 includes a right-tail
  condition in addition to continuous nonnegative density. The sampled
  coverage and report policy are choices in
  [accepted record 20](decisions/financial/models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage).
  [Arbitrage-free SVI volatility surfaces](https://arxiv.org/pdf/1204.0646)

<a id="clark-fx-options"></a>

### Clark FX Options

- Clark, I. *Foreign Exchange Option Pricing*. Reference for FX volatility
  conventions and smile construction.

<a id="wystup-fx-options"></a>

### Wystup FX Options

- Wystup, U. *FX Options and Structured Products*. Reference for delta-based FX
  volatility quoting and smile construction.

<a id="fixed-conversion-quanto-pricing"></a>

### Fixed conversion quanto pricing

- Reiner, E. *Quanto Mechanics*, case 3, defines the fixed contractual
  exchange rate applied to the foreign equity payoff and the domestic
  currency per foreign currency FX quote. [Original article](https://www.academia.edu/16585801/Quanto_mechanics)
- Haugh, M. *Foreign Exchange, ADRs and Quanto-Securities*, Sections 1, 3
  and 4, derives the domestic-measure drift and fixed-conversion option
  premium. With a foreign-price strike, the domestic premium per unit is
  the fixed conversion times the domestic-discounted Black premium.
  [Author's notes](https://www.columbia.edu/~mh2078/ContinuousFE/FX_Quanto.pdf)
- QuantLib's effective yield corroborates the drift sign under matched
  constant inputs. Its term-vol implementation supplies no general
  integrated-covariance identity. [QuantLib source](https://github.com/lballabio/QuantLib/blob/master/ql/termstructures/yield/quantotermstructure.hpp#L112-L121)
  [MODELS-ANALYTIC-07](decisions/financial/models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums)
  selects the checked terminal law and keeps contractual conversion with the
  payout owner.

## Numerical Methods, Statistics, And Randomness

<a id="erdely-2017-bernoulli-subcopula"></a>

### Erdely 2017 Bernoulli Joint Laws

- Erdely, A. (2017). "A Subcopula Based Dependence Measure." *Kybernetika*,
  53(2),231–243. [Primary paper](https://dml.cz/bitstream/handle/10338.dmlcz/146803/Kybernetika_53-2017-2_4.pdf),
  Example3.1, printed p238, for Bernoulli joint cells, Fréchet bounds,
  independence and Pearson correlation. [CORE-NUMERICS-16](decisions/financial/core-numerics/correlated-bernoulli.md)
  selects strict admission and explicit undefined/representation rules; the
  reference does not certify an implementation's rounded cells or sampling.

<a id="huber-maric-2019-bernoulli-correlations"></a>

### Huber–Marić 2019 Admissible Bernoulli Correlations

- Huber, M., and Marić, N. (2019). "Admissible Bernoulli Correlations."
  *Journal of Statistical Distributions and Applications*, 6,article2.
  [Primary article/DOI10.1186/s40488-019-0091-5](https://doi.org/10.1186/s40488-019-0091-5).
  The two-variable Fréchet characterization is separate from multivariate
  compatibility: pairwise feasibility and PSD alone do not guarantee a joint
  Bernoulli law. This supports the ownership limit in CORE-NUMERICS-16, not an
  automatic projection or financial dependence-model choice.

<a id="laurent-massart-2000-chi-squared"></a>

### Laurent–Massart 2000 Chi-Squared Concentration

- Laurent, B., and Massart, P. (2000). "Adaptive Estimation of a Quadratic
  Functional by Model Selection." *Annals of Statistics*, 28(5),1302–1338.
  [DOI: 10.1214/aos/1015957395](https://doi.org/10.1214/aos/1015957395),
  section4.1/equation4.3 for an integer-dimension chi-squared upper-tail bound.
  The real-df Gamma-MGF extension and wider numerical bracket in
  [CORE-NUMERICS-15](decisions/financial/core-numerics/chi-squared-inversion.md)
  are derived there explicitly; the concentration result does not establish
  floating-point inversion accuracy or finite-budget success.

<a id="marsaglia-tsang-2000-gamma"></a>

### Marsaglia–Tsang 2000 Gamma Sampling

- Marsaglia, G., and Tsang, W. W. (2000). "A Simple Method for Generating Gamma
  Variables." *ACM Transactions on Mathematical Software*, 26(3), 363–372.
  [DOI: 10.1145/358407.358414](https://doi.org/10.1145/358407.358414);
  [coauthor institutional record/abstract](https://hub.hku.hk/handle/10722/89056).
  Primary provenance for the normal/cubic rejection method, squeeze and
  positive-shape augmentation. The checked finite-RNG/component/budget profile
  in [CORE-NUMERICS-14](decisions/financial/core-numerics/beta-gamma-sampling.md)
  requires independent numerical/statistical qualification; the ideal method
  does not certify every finite implementation or recovery model.

<a id="devroye-1986-random-variates"></a>

### Devroye 1986 Non-Uniform Random Variate Generation

- Devroye, L. (1986). *Non-Uniform Random Variate Generation*. Springer.
  [Author-hosted chapter IX](https://luc.devroye.org/chapter_nine.pdf),
  theorem3.1 (printed pp402–403) for independent-Gamma beta ratios and printed
  p420 for positive-shape augmentation. The distributional identities support
  the selected sampler construction, not endpoint approximations, unchecked
  numerical floors or a production tail/accuracy guarantee.
  [Author-hosted chapter V](https://luc.devroye.org/chapter_five.pdf), section4.4,
  printed pp235–236, derives the Box–Muller normal transform used as ideal-law
  provenance in CORE-NUMERICS-17; finite grids and project endpoint/cache profiles
  need their own qualification.

<a id="nist-binomial"></a>

### Binomial Law and Mode

- NIST/SEMATECH *e-Handbook of Statistical Methods*,
  [Binomial Distribution](https://www.itl.nist.gov/div898/handbook/eda/section3/eda366i.htm):
  PMF, support, common per-trial probability and mode inequality. The
  [Boost binomial documentation](https://www.boost.org/doc/libs/latest/libs/math/doc/html/math_toolkit/dist_ref/dists/binomial_dist.html)
  provides a maintained implementation reference; [R's binomial documentation](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/Binomial.html)
  distinguishes probability from direct logarithmic evaluation.
  [CORE-NUMERICS-13](decisions/financial/core-numerics/binomial-mass.md) selects a
  derived full-array mode-relative recurrence with checked normalization. Its
  resource cap, numerical admission gate and arithmetic/error policy are project
  decisions; these sources do not certify its per-cell/tail accuracy or credit
  model suitability.

<a id="nist-special-functions"></a>

### NIST DLMF Special Functions

- NIST *Digital Library of Mathematical Functions*: [Gamma definition](https://dlmf.nist.gov/5.2),
  [Gamma recurrence](https://dlmf.nist.gov/5.5.E1),
  [Gamma increment series](https://dlmf.nist.gov/5.7),
  [Stirling expansion and remainders](https://dlmf.nist.gov/5.11),
  [error-function definitions](https://dlmf.nist.gov/7.2),
  [continued fractions](https://dlmf.nist.gov/7.9) and
  [erfc asymptotic expansion/remainder](https://dlmf.nist.gov/7.12.E1).
  For chi-squared inversion, [NIST's central-law CDF](https://www.itl.nist.gov/div898/handbook/eda/section3/eda3666.htm),
  [incomplete-Gamma definitions](https://dlmf.nist.gov/8.2),
  [special values](https://dlmf.nist.gov/8.4),
  [lower series](https://dlmf.nist.gov/8.7.E1),
  [shift recurrence](https://dlmf.nist.gov/8.8.E5) and
  [upper fraction](https://dlmf.nist.gov/8.9.E2) identify the exact functions.
  Primary mathematical provenance for the selected real functions and bounded
  reference calculations in [CORE-NUMERICS-12](decisions/financial/core-numerics/distribution-functions.md)
  and [CORE-NUMERICS-15](decisions/financial/core-numerics/chi-squared-inversion.md).
  Exact-function identities and series remainder bounds do not certify the total
  floating error, convergence or financial suitability of an implementation.

<a id="statrs-018-special-functions"></a>

### statrs 0.18 Special-Function Profiles

- statrs0.18.0 [normal](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/distribution/normal.rs),
  [erf](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/function/erf.rs),
  [Gamma](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/function/gamma.rs),
  [Student-t](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/distribution/students_t.rs)
  and [beta](https://github.com/statrs-dev/statrs/blob/v0.18.0/src/function/beta.rs)
  source; version pinned in [Cargo.lock](../Cargo.lock). These identify the actual
  Boost-derived inverse-error-function and AS64/AS109 inverse-beta profiles,
  including dependency stopping/range limitations. The owning
  [distribution decision](decisions/financial/core-numerics/distribution-functions.md)
  selects checked domains, outputs and a narrowly bounded private Student-t
  adaptation; dependency reuse alone supplies no accuracy or work certificate.

<a id="higham-accuracy-and-stability"></a>

### Higham Accuracy And Stability

- Higham, N. J. *Accuracy and Stability of Numerical Algorithms*. Canonical
  reference for floating-point error analysis and numerically stable algorithms.

<a id="higham-2002-nearest-correlation"></a>

### Higham 2002 Nearest Correlation Matrix

- Higham, N. J. (2002). *Computing the nearest correlation matrix—A problem
  from finance*. IMA Journal of Numerical Analysis, 22(3), 329–343.
  [DOI: 10.1093/imanum/22.3.329](https://doi.org/10.1093/imanum/22.3.329);
  [author's paper](https://eprints.maths.manchester.ac.uk/232/1/paper3.pdf).
  Canonical reference for the Frobenius nearest-correlation problem and
  modified alternating projections with Dykstra correction. The paper's
  displayed stopping quantities use infinity norms; the
  [author's later implementation](https://nhigham.com/2013/02/13/the-nearest-correlation-matrix/)
  uses three relative Frobenius quantities. Finite stopping is not a certified
  objective-gap or exact-PSD guarantee.

<a id="higham-1993-summation"></a>

### Higham 1993 Floating Point Summation

- Higham, N. J. (1993). *The Accuracy of Floating Point Summation*. SIAM
  Journal on Scientific Computing, 14(4), 783–799.
  [Author's paper](https://nhigham.com/wp-content/uploads/2023/10/high93s.pdf).
  Reference for ordered floating-point summation, compensation and conditional
  error analysis; no method is uniformly more accurate for every sequence.

<a id="hyndman-fan-1996-quantiles"></a>

### Hyndman And Fan 1996 Sample Quantiles

- Hyndman, R. J., and Fan, Y. (1996). *Sample Quantiles in Statistical Packages*.
  The American Statistician, 50(4), 361–365.
  [Author's page and errata](https://robjhyndman.com/publications/quantiles/);
  [DOI: 10.1080/00031305.1996.10473566](https://doi.org/10.1080/00031305.1996.10473566).
  Reference for sample-quantile definitions; the selected R7 convention is also
  specified in the [official R manual](https://www.stat.ethz.ch/R-manual/R-patched/library/stats/html/quantile.html).
  Estimator agreement does not adopt another package's range/missing-data/fuzz rules.

<a id="pebay-2008-moments"></a>

### Pebay 2008 Parallel Centered Moments

- Pébay, P. P. (2008). *Formulas for Robust, One-Pass Parallel Computation of
  Covariances and Arbitrary-Order Statistical Moments*. Sandia report
  SAND2008-6212. [Sandia publication record](https://www.sandia.gov/research/publications/details/formulas-for-robust-one-pass-parallel-computation-of-covariances-and-arbitr-2008-09-01/).
  Reference for incremental/parallel centered-moment and covariance algebra,
  not a universal finite-range or rounding-accuracy guarantee.

<a id="parkinson-1980-range-variance"></a>

### Parkinson 1980 Range Variance

- Parkinson, M. (1980). *The Extreme Value Method for Estimating the Variance
  of the Rate of Return*. Journal of Business, 53(1), 61–65.
  [DOI: 10.1086/296071](https://doi.org/10.1086/296071).
  Reference for the high–low squared-log-range estimator under its diffusion
  assumptions, not universal efficiency for discretely recorded market extrema.

<a id="garman-klass-1980-range-variance"></a>

### Garman And Klass 1980 Range Variance

- Garman, M. B., and Klass, M. J. (1980). *On the Estimation of Security Price
  Volatilities from Historical Data*. Journal of Business, 53(1), 67–78.
  [DOI: 10.1086/296072](https://doi.org/10.1086/296072);
  [author manuscript hosted by CME](https://www.cmegroup.com/trading/fx/files/a_estimation_of_security_price.pdf).
  Distinguish the selected practical estimator (19a) from the full optimized
  quadratic (19) and opening-gap extension (20).

<a id="rogers-satchell-1991-range-variance"></a>

### Rogers And Satchell 1991 Range Variance

- Rogers, L. C. G., and Satchell, S. E. (1991). *Estimating Variance from High,
  Low and Closing Prices*. Annals of Applied Probability, 1(4), 504–512.
  [DOI: 10.1214/aoap/1177005835](https://doi.org/10.1214/aoap/1177005835);
  [author paper](https://www.skokholm.co.uk/wp-content/uploads/2016/01/R_Satchell_HLOC.pdf).
  Drift independence is model-conditional; discrete recorded extrema introduce
  a separate estimation error and opening gaps require their own treatment.

<a id="yang-zhang-2000-range-variance"></a>

### Yang And Zhang 2000 Range Variance

- Yang, D., and Zhang, Q. (2000). *Drift-Independent Volatility Estimation
  Based on High, Low, Open, and Close Prices*. Journal of Business, 73(3),
  477–492. [DOI: 10.1086/209650](https://doi.org/10.1086/209650);
  [paper](https://www.atmif.com/papers/range.pdf).
  Reference for complete periods with a preceding close, overnight/intraday
  sample variances and mean Rogers–Satchell. The selected practical weight
  (18) fixes alpha=1.34; the general weight (10) is parameter-dependent.

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

<a id="expression-three-valued-logic"></a>

### Expression three-valued logic

- PostgreSQL 18's official [logical truth tables](https://www.postgresql.org/docs/18/functions-logical.html)
  and [unknown comparison rules](https://www.postgresql.org/docs/18/functions-comparison.html)
  provide an established NULL-valued comparison/strong-three-valued-logic
  reference. CORE-NUMERICS-22 selects those value tables with runtime NaN as its
  own missing carrier; it does not adopt PostgreSQL floating NaN semantics,
  SQL syntax/optimizer/order or statements-evaluator compatibility.

<a id="binary64-decimal-quantization"></a>

### Exact decimal quantization of binary64 values

- [num-rational 0.4.2 source](https://raw.githubusercontent.com/rust-num/num-rational/num-rational-0.4.2/src/lib.rs)
  supplies exact from_float decomposition, Ratio::round with half-away integer
  ties, and ratio_to_f64 with guard/remainder nearest-even restoration.
  [Core dependencies](../finstack-quant/core/Cargo.toml) and the
  [lockfile](../Cargo.lock) already include this version. CORE-NUMERICS-22 uses
  these bounded existing primitives for exact binary-input decimal quantization
  and rejects nonfinite final restoration. Its full-i32 tails, language missing/
  error controls and financial-owner boundaries are separately selected project
  policy; source inspection is not executed production qualification or Money
  rounding/Excel compatibility.

<a id="joe-kuo-2008-sobol"></a>

### Joe Kuo 2008 Sobol

- Joe, S., and Kuo, F. Y. "Constructing Sobol Sequences with Better
  Two-Dimensional Projections." *SIAM Journal on Scientific Computing*, 30(5),
  2635-2654. [Author data page](https://web.maths.unsw.edu.au/~fkuo/sobol/)
  and [construction notes](https://web.maths.unsw.edu.au/~fkuo/sobol/joe-kuo-notes.pdf).
  CORE-NUMERICS-18 identifies the D6 `new-joe-kuo-6.21201` subset for dimensions
  2..40, rather than an interchangeable Joe–Kuo-labelled table. Its existing
  [golden-data record](../finstack-quant/core/tests/data/README_sobol.md)
  provides retrieval date/full-file checksum; subset agreement is not fresh
  authentication of that full author file. Natural binary and Gray point order
  require explicit distinction.

<a id="sobol-1967"></a>

### Sobol 1967

- Sobol, I. M. "Distribution of points in a cube and approximate evaluation of
  integrals." *USSR Computational Mathematics and Mathematical Physics*, 7(4),
  86-112. Original Sobol sequence.

<a id="owen-1995-scrambling"></a>

### Owen 1995 Scrambling

- Owen, A. B. "Randomly Permuted (t,m,s)-Nets and (t,s)-Sequences." Monte Carlo
  and Quasi-Monte Carlo Methods in Scientific Computing, 299-317. Canonical
  random-permutation theory; this citation does not establish the project's
  deterministic prefix-hash seed law, unbiasedness or variance guarantees.

<a id="owen-2021-sobol-first-point"></a>

### Owen 2021 Sobol first point

- Owen, A. B. "On dropping the first Sobol' point."
  [arXiv:2008.08051v4](https://arxiv.org/abs/2008.08051v4), revised 2021-12-12.
  Explains how first-point removal, thinning and inappropriate sample counts
  can damage net structure. Independent replicate means, rather than within-net
  points, are the units for replicate uncertainty under the relevant assumptions;
  this paper does not qualify the project's prefix-hash family or owner policy.

### Sobol blocks, Brownian paths and replicate diagnostics

- Owen analyzes how thinning, skipping point zero and inappropriate counts
  can degrade a Sobol net. The accepted pricing mode takes complete,
  equal base-2 prefixes from point zero. The selected prefix-hash
  profile has the finite net properties stated in CORE-NUMERICS-18.
  [On Dropping the First Sobol' Point](https://arxiv.org/abs/2008.08051).
- Morokoff describes Brownian bridge construction for quasi-random paths.
  Wang and Sloan show that its benefit depends on the payoff. The accepted
  pricing mode evaluates standardized increments using the conditional
  Gaussian split on actual knots. [Generating Quasi-Random Paths for Stochastic Processes](https://epubs.siam.org/doi/10.1137/S0036144597317959),
  [Brownian bridge performance in finance](https://pubsonline.informs.org/doi/10.1287/opre.1100.0853).
- L'Ecuyer et al. study Student-t intervals over independently randomized
  replicate estimates. The accepted deterministic root-to-seed profile
  supplies descriptive replicate spread and leaves the confidence interval
  and sampling standard error unavailable. [Confidence Intervals for Randomized Quasi-Monte Carlo Estimators](https://www.informs-sim.org/wsc23papers/037.pdf).

<a id="box-muller-1958"></a>

### Box Muller 1958

- Box, G. E. P., and Muller, M. E. "A Note on the Generation of Random Normal
  Deviates." *Annals of Mathematical Statistics*, 29(2),610–611 (1958).
  [DOI: 10.1214/aoms/1177706645](https://doi.org/10.1214/aoms/1177706645).
  Polar/Box–Muller ideal Gaussian transform; CORE-NUMERICS-17's reflected
  half-open binary64 inputs/cache/error choices require separate qualification.

<a id="brownian-bridge-conditional-law"></a>

### Conditional Brownian Bridge Evaluation

- [QMCPy's official Brownian bridge demonstration](https://qmcsoftware.org/qmcpy/demos/brownian_bridge/)
  supplies a maintained implementation reference for conditional Gaussian path
  construction. The terminal-first DFS index mapping, bounded topology, stable
  arithmetic and scratch/error contract in [CORE-NUMERICS-17](decisions/financial/core-numerics/pseudorandom-and-bridge.md)
  are project choices. The reference does not certify global variance ordering,
  every finite-grid payoff, QMC convergence rates or continuous barrier monitoring.

<a id="poisson-count-quantile"></a>

### Poisson Count Quantiles

- [R's official Poisson documentation](https://stat.ethz.ch/R-manual/R-patched/library/stats/html/Poisson.html)
  defines the smallest-count CDF inverse and expected-count parameter.
  [Boost's Poisson documentation](https://www.boost.org/doc/libs/latest/libs/math/doc/html/math_toolkit/dist_ref/dists/poisson_dist.html)
  identifies the regularized-Gamma CDF/survival relations. The project selects its
  own checked bounded integer search and exact count domain in CORE-NUMERICS-17;
  neither source's complete inversion/rounding policy or runtime implementation
  is adopted as a numerical oracle.

<a id="salmon-2011-philox"></a>

### Salmon 2011 Philox

- Salmon, J. K., Moraes, M. A., Dror, R. O., and Shaw, D. E. "Parallel Random
  Numbers: As Easy as 1, 2, 3." *SC '11*. Counter-based Philox generator used
  for splittable Monte Carlo streams. The Random123 vector file supplies
  independent all-zero, all-ones and mixed key/counter checks. The project
  selects its own word pairing, midpoint mapping, normal transform and
  exhaustion policy.
  [Original paper](https://www.thesalmons.org/john/random123/papers/random123sc11.pdf),
  [Random123 known-answer vectors](https://github.com/DEShawResearch/random123/blob/main/tests/kat_vectors).

### FNV-1a seed derivation and UTF-8 framing

- Noll et al. specify FNV-1a's XOR-before-multiply recurrence and the
  64-bit constants. The project's instrument-and-scenario mapping uses
  its own byte framing. [FNV, RFC 9923 §§2 and 5](https://www.rfc-editor.org/rfc/rfc9923.html#section-2).
- Yergeau documents the permitted UTF-8 byte sequences, which exclude
  `0xff`. The delimiter therefore identifies the component boundary
  before hashing. A 64-bit hash can map distinct encoded pairs to the
  same seed. [UTF-8, RFC 3629 §1](https://www.rfc-editor.org/rfc/rfc3629.html#section-1).

<a id="o-neill-2014-pcg"></a>

### O'Neill 2014 PCG

- O'Neill, M. E. "PCG: A Family of Simple Fast Space-Efficient Statistically
  Good Algorithms for Random Number Generation." (2014).
  [Author's paper page](https://www.pcg-random.org/paper.html). Permuted
  congruential generators; distinguish the family reference from the pinned
  rand_pcg implementation and CORE-NUMERICS-17's explicit initialization/grid
  profile. Distinct streams, seeds and long period alone establish neither
  empirical independence nor non-overlapping consumed windows.

<a id="mandelbrot-van-ness-1968"></a>

### Mandelbrot Van Ness 1968

- Mandelbrot, B., and Van Ness, J. "Fractional Brownian Motions, Fractional
  Noises and Applications." *SIAM Review*, 10(4), 422-437 (1968).
  [DOI/primary paper](https://epubs.siam.org/doi/pdf/10.1137/1010093).
  Stationary/self-similar increments (Theorem 3.3), amplitude convention
  (Corollary 3.4), separated-increment covariance (section 5.1) and
  nondifferentiability (Proposition 4.2). CORE-NUMERICS-19 explicitly selects
  unit variance at time one; its arithmetic/gate and nonnegative-time admission
  are project choices, not the unmodified moving-average normalization or a
  financial-model recommendation.

<a id="gao-liu-oosterlee-temme-2023-fbm"></a>

### Gao Liu Oosterlee Temme 2023 fBM

- Gao, F., Liu, S., Oosterlee, C. W., and Temme, N. M. "Evaluation of integrals
  with fractional Brownian motion for different Hurst indices." *International
  Journal of Computer Mathematics* (2023), DOI: 10.1080/00207160.2022.2163166.
  [Author/institutional paper](https://ir.cwi.nl/pub/32819/32819.pdf), equation 1,
  explicitly states the standard unit-normalized covariance and Brownian limit.
  This source anchors that mathematical definition, not the project's numerical
  cancellation gate, rounded PSD, sampler policy or financial accuracy.

<a id="in-t-hout-welfert-2009"></a>

### In 't Hout Welfert 2009

- In 't Hout, K. J., and Welfert, B. D. "Unconditional stability of second-order
  ADI schemes applied to multi-dimensional diffusion equations with mixed
  derivative terms." *Applied Numerical Mathematics*, 59(3–4), 677–692.
  The paper introduces Modified Craig–Sneyd and analyzes diffusion
  discretizations with mixed derivatives.
  [Primary paper](https://www.sciencedirect.com/science/article/abs/pii/S016892740800055X)

<a id="craig-sneyd-1988"></a>

### Craig and Sneyd 1988

- Craig and Sneyd introduce the earlier alternating-direction
  implicit scheme with a mixed-derivative correction.
  [An Alternating-Direction Implicit Scheme for Parabolic Equations with Mixed Derivatives](https://www.sciencedirect.com/science/article/pii/0898122188901502)

<a id="in-t-hout-mishra-mcs-stability"></a>

### In 't Hout and Mishra MCS stability

- Equation (1.4) gives the full MCS stages. Theorem 2.2 gives the
  real-symbol \(1/3\) result. Theorem 2.3 establishes \(2/5\)
  as necessary for its broader symbol class and conjectures
  sufficiency. Section 3 states the Fourier problem assumptions.
  [Stability of the Modified Craig–Sneyd Scheme for Two-Dimensional Convection-Diffusion Equations with Mixed Derivative Term](https://arxiv.org/pdf/1011.6528)

### Checked PDE grids, interpolation and coordinate Greeks

- In 't Hout and Foulon use Cartesian sinh meshes in Section 2.2,
  equations (2.6)–(2.8), and a barrier endpoint in Section 3.3.
  [Record 011](decisions/financial/models-numerical/pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)
  uses the mapping family in the chosen model coordinate, with
  checked finite geometry and explicit endpoints.
  [ADI Finite Difference Schemes for Option Pricing in the Heston Model with Correlation](https://www.math.ualberta.ca/ijnam/Volume-7-2010/No-2-10/2010-02-06.pdf)
- Fornberg derives polynomial finite-difference weights on arbitrary
  1D meshes. The accepted nodal first and second derivatives use
  the three-point quadratic case, including endpoint evaluations.
  The closed-domain rule and interpolation of derivative estimates
  are project choices. Product accuracy requires independent refinement
  and reference comparisons.
  [Generation of Finite Difference Formulas on Arbitrarily Spaced Grids](https://www.ams.org/mcom/1988-51-184/S0025-5718-1988-0935077-0/S0025-5718-1988-0935077-0.pdf)

### Checked directional and mixed PDE stencils

- Windcliff, Forsyth and Vetzal derive the nonuniform central row
  and switch to upstream convection when an off-diagonal turns
  negative, in Section 4, equations (4.2)–(4.7).
  [Record 012](decisions/financial/models-numerical/pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils)
  selects that switch with exact product comparison and checked
  row arithmetic. Boundary and time-march decisions have separate scope.
  [Analysis of the Stability of the Linear Boundary Condition for the Black–Scholes Equation](https://cs.uwaterloo.ca/~paforsyt/bc.pdf)
- In 't Hout and Foulon give the centered directional weights
  in equations (2.9b) and (2.10) and the tensor mixed stencil
  in (2.11). Section 2.3 treats the mixed term separately from
  implicit directional solves. The accepted equal split of
  the source and the strict represented-coefficient admission
  are specified in 012. The paper's upwind rule and boundary
  closure use their own assumptions.
  [ADI Finite Difference Schemes for Option Pricing in the Heston Model with Correlation](https://www.math.ualberta.ca/ijnam/Volume-7-2010/No-2-10/2010-02-06.pdf)

### Checked PDE boundary elimination and reconstruction

- Windcliff, Forsyth and Vetzal discuss the zero spot-curvature
  condition and the role of the boundary row in stability in
  Sections 2.2 and 4.1. [Record 013](decisions/financial/models-numerical/pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)
  uses the affine-in-spot idea for log-spot extrapolation and
  defines its own checked local substitution.
  [Analysis of the Stability of the Linear Boundary Condition for the Black–Scholes Equation](https://cs.uwaterloo.ca/~paforsyt/bc.pdf)
- In 't Hout and Foulon use a limiting PDE treatment at the
  zero-variance edge under their Feller assumption in Sections
  2.1 and 2.2. The project's corner ownership and affine
  boundary relations are choices in 013. Heston closure selection
  and qualification belong to the bridge record.
  [ADI Finite Difference Schemes for Option Pricing in the Heston Model with Correlation](https://www.math.ualberta.ca/ijnam/Volume-7-2010/No-2-10/2010-02-06.pdf)

### Checked theta marching and tridiagonal solves

- Giles and Carter analyze the convergence of damped
  Crank–Nicolson on their selected problems. Section 4.3
  distinguishes full backward-Euler intervals from
  Rannacher's paired halfsteps. [Record 014](decisions/financial/models-numerical/theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)
  selects the full-interval profile and tracks independent
  qualification of the live product counts and event restarts.
  [Convergence Analysis of Crank–Nicolson and Rannacher Time-Marching](https://people.maths.ox.ac.uk/~gilesm/files/giles_carter.pdf)
- Higham gives the tridiagonal elimination recurrences in
  Section 2 and qualifies error bounds by matrix class.
  LAPACK's DGTRFS documents the componentwise backward
  error based on the original row and RHS. Record 014
  specifies its own scaled ratio and inclusive tolerance.
  [Bounding the Error in Gaussian Elimination for Tridiagonal Systems](https://nhigham.com/wp-content/uploads/2023/09/high90t-1.pdf),
  [LAPACK DGTRFS](https://netlib.org/lapack/explore-html/d2/d33/dgtrfs_8f_source.html)

### Checked grid-based PDE exercise

- In 't Hout and Valkov give the explicit payoff split in Section 3,
  equation (7), and report regime-dependent temporal errors.
  In 't Hout and Snoeijer give the Bermudan exercise-time maximum
  in equations (1.3) and (2.13). [Record 015](decisions/financial/models-numerical/pde-exercise-projection.md#accepted-decision-for-checked-grid-based-pde-exercise)
  selects the nodewise rule for its qualified 1D profile.
  [Numerical Study of Splitting Methods for American Option Valuation](https://arxiv.org/pdf/1610.09622),
  [Numerical Valuation of Bermudan Basket Options via Partial Differential Equations](https://arxiv.org/pdf/1909.01164)
- Forsyth and Vetzal distinguish explicit obstacle enforcement from
  a coupled penalty iteration and publish independent American put
  comparators in Tables 11.1 and 12.1. Their coupled solve and binomial
  estimates provide qualification targets with aligned inputs.
  [Quadratic Convergence for Valuing American Options Using a Penalty Method](https://cs.uwaterloo.ca/~paforsyt/con7.pdf)

<a id="checked-modified-craig-sneyd-adi"></a>

### Checked Modified Craig–Sneyd ADI

- [Record 016](decisions/financial/models-numerical/mcs-adi-time-march.md#accepted-decision-for-checked-modified-craigsneyd-adi)
  selects fixed-parameter MCS and the project's factorized startup.
  In 't Hout and Foulon give the MCS stages in Section 2.3
  and European Heston input sets and refinement comparisons
  in Table 1 and Sections 3.1–3.2. Their backward-Euler
  startup comparisons use a coupled step.
  [In 't Hout and Foulon](https://www.math.ualberta.ca/ijnam/Volume-7-2010/No-2-10/2010-02-06.pdf)
- Wyns analyzes MCS with coupled backward-Euler halfsteps
  under the paper's model and grid assumptions. The
  factorized startup in 016 has different mixed-term and
  affine stage effects and requires independent qualification.
  [Convergence Analysis of the Modified Craig–Sneyd Scheme with Nonsmooth Initial Data](https://arxiv.org/pdf/1508.04296)

### Checked PDE coefficient bridges

- Black and Scholes give the constant-volatility pricing PDE.
  Heston gives the square-root variance model, mixed PDE term
  and characteristic-function pricing. [Record 017](decisions/financial/models-numerical/black-scholes-and-heston-pde-bridges.md#accepted-decision-for-checked-pde-coefficient-bridges)
  selects checked generators in log spot and variance with
  explicit risk-neutral inputs and represented diffusion rules.
  [The Pricing of Options and Corporate Liabilities](https://www.journals.uchicago.edu/doi/10.1086/260062),
  [Heston's stochastic volatility model](https://www.ma.imperial.ac.uk/~ajacquie/IC_Num_Methods/IC_Num_Methods_Docs/Literature/Heston.pdf)
- In 't Hout and Foulon describe the vanishing diffusion terms
  at zero variance and their limiting-PDE treatment under strict
  Feller in Sections 2.1 and 2.2. Record 017 selects linear
  reconstruction for the existing solver topology and requires
  separate reachable-zero, zero-start and truncation evidence.
  [ADI Finite Difference Schemes for Option Pricing in the Heston Model with Correlation](https://www.math.ualberta.ca/ijnam/Volume-7-2010/No-2-10/2010-02-06.pdf)

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
  88(2), 365–411. [DOI: 10.1016/S0047-259X(03)00096-4](https://doi.org/10.1016/S0047-259X(03)00096-4);
  [primary paper](https://perso.ens-lyon.fr/patrick.flandrin/LedoitWolf_JMA2004.pdf).
  Scaled-identity linear covariance shrinkage with an estimated intensity and
  asymptotic quadratic-risk optimality under the paper's IID, moment and
  dimensional conditions, not a finite-sample oracle or universal conditioning
  guarantee. The estimated-mean population-normalized plug-in profile in
  `finstack_quant_core::math::linalg::ledoit_wolf_shrinkage` and its
  `CovarianceStrategy::LedoitWolf` credit consumer require separate numerical
  and financial qualification; see the [owning decision](decisions/financial/core-numerics/covariance-shrinkage.md).

<a id="artzner1999CoherentRisk"></a>

### Artzner 1999 Coherent Risk

- Artzner, P., Delbaen, F., Eber, J.-M., and Heath, D. "Coherent Measures of
  Risk." Canonical reference for Expected Shortfall as a coherent risk measure.

<a id="rockafellarUryasevGeneralLoss"></a>

### Rockafellar Uryasev General Loss Distributions

- Rockafellar, R. T., and Uryasev, S. [*Conditional Value-at-Risk for General
  Loss Distributions*](https://sites.math.washington.edu/~rtr/papers/rtr187-CVaR2.pdf),
  author manuscript dated November 28, 2001. Definition 3 and Propositions
  6 and 8 specify tail probability weighting, including splitting a boundary
  probability atom. Analytics uses the corresponding lower-return tail and
  retains its separately selected R-7 VaR convention.

<a id="fisherCornish1960"></a>

### Fisher Cornish 1960

- Fisher, R. A., and Cornish, E. A. ["The Percentile Points of Distributions
  Having Known Cumulants"](https://www.stat.cmu.edu/technometrics/59-69/VOL-02-02/v0202209.pdf).
  *Technometrics* 2(2), 209–225, May 1960. Primary quantile-expansion context
  and polynomial tables. Analytics explicitly specifies its retained cubic,
  Type 2 sample plug-in coefficients and analytic global derivative gate;
  this reference does not certify those plug-in estimates or model accuracy.

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

- Martin, P. G. ["Ulcer Index: An Alternative Approach to the Measurement
  of Investment Risk & Risk-Adjusted Performance"](https://www.tangotools.com/ui/ui.htm).
  The author identifies development in 1987 and first publication with
  Byron McCann in *The Investor's Guide to Fidelity Funds* (Wiley, 1989).
  His mean-squared percentage-drawdown definition and cash-adjusted
  UPI/Martin formula support the selected measures; observation weighting,
  units and dated-CAGR convention are explicit analytics choices.

<a id="youngCalmar1991"></a>

### Young 1991 Calmar

- Young, T. W. "Calmar Ratio: A Smoother Tool." Practitioner reference for the
  Calmar ratio.

<a id="kestner2003"></a>

### Kestner 2003

- Kestner, L. N. *Quantitative Trading Strategies*, first edition. McGraw-Hill,
  2003, ISBN 9780071412391. The [publisher catalogue](https://www.mheducation.com/highered/mhp/product/quantitative-trading-strategies.html)
  establishes the edition and publication year. This is trading-strategy
  context; the selected worst-episode-average Sterling formula is specified
  separately in [analytics record 06](decisions/financial/analytics/06-drawdown-reward-and-cdar.md).

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

### Autocallable observation eligibility and payment terms

- The Royal Bank of Canada Buffered Phoenix pricing supplement
  specifies separate coupon and call conditions, excludes the final
  observation from calling and releases memory only when the coupon
  condition is met. Its buffered downside is specific to that issue.
  [RBC filed pricing supplement](https://www.sec.gov/Archives/edgar/data/1000275/000095010325000733/dp223672_424b2-jpmeln550.htm).
- The UBS contingent-income pricing supplement illustrates
  separate final determination and maturity dates, memory released
  at the qualifying payment date and inclusive level tests.
  [UBS filed pricing supplement](https://www.sec.gov/Archives/edgar/data/1114446/000183988225039448/ubs_424b2-21451.htm).
- The Nasdaq-100 accelerated-redemption filing includes the final
  observation as an eligible call with maturity-date settlement.
  Issuer terms govern actual observations, call amounts and
  redemption. The [accepted autocallable record](decisions/financial/models-monte-carlo/20-autocallable.md#accepted-decision-for-checked-monte-carlo-autocallable-payoffs)
  selects a single-underlying par-call contract with explicit row
  eligibility and its own three maturity redemption formulas.
  [Filed accelerated-redemption terms](https://www.sec.gov/Archives/edgar/data/83246/000110465925103049/tm2527726d407_fwp.pdf).

### Cliquet reset and accumulation contracts

- The historical CBOE FLEX approval order describes a standalone
  cliquet option using successive monthly fixing levels, capped
  simple returns, addition and a final zero floor.
  [CBOE approval order](https://www.sec.gov/rules/sro/cboe/2015/34-75425.pdf).
- Barclays' filed pricing supplement describes the initial level
  and subsequent period-end anchors for an additive capped-return
  note. That issue specifies principal separately.
  [Barclays filed pricing supplement](https://www.sec.gov/Archives/edgar/data/312070/000119312511020240/d424b2.htm).
- Carr and Wu, equations 19–20, use a product of locally capped
  gross factors for a compounding cliquet. Their example pays gross
  value. The [accepted Monte Carlo cliquet record](decisions/financial/models-monte-carlo/21-cliquet.md#accepted-decision-for-checked-monte-carlo-cliquet-payoffs)
  uses the recurrence and specifies its own return-only payout.
  [Static Hedging of Standard Options](https://engineering.nyu.edu/sites/default/files/2019-01/CarrStandardOptionsJuly2002-a.pdf).

### Range accrual observation counts and coupon terms

- Bank of America's RANGE-1 supplement defines a qualifying-day
  count over the full applicable count. Its term sheets choose the
  eligible day basis, period boundaries and range.
  [Filed range accrual supplement](https://www.sec.gov/Archives/edgar/data/70858/000119312510199455/d424b5.htm).
- JPMorgan's equity-index supplement describes inclusive lower and
  upper band tests where selected by the issue terms.
  [Filed equity-index range terms](https://www.sec.gov/Archives/edgar/data/19617/000089109214000661/e57163_424b2.pdf).
- Morgan Stanley's buffered issue illustrates an annual rate,
  a qualifying index-business-day fraction, a separate 30/360
  basis and distinct determination and payment dates. The
  [accepted range accrual record](decisions/financial/models-monte-carlo/22-range-accrual.md#accepted-decision-for-checked-monte-carlo-range-accrual)
  accepts a prepared full-period coupon for one fixed schedule.
  [Filed buffered range accrual terms](https://www.sec.gov/Archives/edgar/data/895421/000095010312001632/dp29638_424b2-ps133.htm).

## Expected Credit Loss And Accounting Impairment

<a id="ifrs-9-impairment"></a>

### IFRS 9 Impairment

- IFRS Foundation. *IFRS 9 Financial Instruments*, Section 5.5. Expected-credit-
  loss staging and measurement used by `classify_stage_from_curves` and
  `compute_ecl_weighted_from_curves`.

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
