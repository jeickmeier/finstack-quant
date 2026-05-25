# Cross-Asset Instrument & Feature Checklist

Use this checklist to identify coverage gaps. Items marked with priority indicate how commonly they appear on a professional trading desk.

**Priority key**: P1 = must-have for any serious library, P2 = expected by most desks, P3 = nice-to-have / niche

---

## Interest Rates

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Cash deposit | P1 | Money market rate |
| FRA | P1 | Forward rate agreement |
| IRS (fixed/float) | P1 | Plain vanilla, OIS, SOFR, EURIBOR |
| Basis swap | P1 | Float/float, tenor basis |
| Cross-currency swap | P1 | Resettable notional, mark-to-market |
| Amortizing swap | P2 | Custom amortization schedules |
| Zero-coupon swap | P2 | Single payment at maturity |
| OIS | P1 | Overnight indexed swap |
| Compounding swap | P2 | Compounded in-arrears SOFR |
| Cap/floor | P1 | Caplet/floorlet decomposition |
| Collar | P1 | Long cap + short floor |
| Swaption (European) | P1 | Physical and cash settlement |
| Swaption (Bermudan) | P1 | Requires tree or LSM |
| Swaption (American) | P3 | Rare, usually Bermudan |
| CMS swap | P2 | Constant maturity swap |
| CMS cap/floor | P2 | Options on CMS rates |
| Range accrual | P2 | Accrual on rate corridor |
| Callable/putable swap | P2 | Embedded optionality |
| Inflation swap (zero-coupon) | P2 | CPI-linked |
| Inflation swap (YoY) | P2 | Year-on-year |
| Inflation cap/floor | P2 | Options on realized inflation |
| Bond future | P1 | CTD analysis, delivery option |
| IR future (STIR) | P1 | Eurodollar, SOFR futures |
| IR future option | P2 | Options on STIR futures |
| Repo / reverse repo | P1 | Funding instrument |

### Pricing Models

| Model | Priority | Instruments |
|-------|----------|-------------|
| Discounting (analytic) | P1 | Deposits, FRA, vanilla swaps |
| Black-76 | P1 | Caps, floors, European swaptions |
| Bachelier (normal) | P1 | Common swaption quoting convention, especially in low/negative-rate regimes |
| SABR | P1 | Vol surface interpolation |
| Hull-White 1F | P1 | Bermudan swaptions, callable bonds |
| Hull-White 2F | P2 | Better correlation structure |
| LGM (Linear Gaussian Markov) | P2 | Equivalent to HW1F but better calibration |
| Shifted SABR | P2 | Handle negative rates in SABR |
| Shifted Black | P2 | Handle negative rates in Black |
| Markov-functional | P3 | Advanced Bermudan pricing |
| BGM/LMM | P3 | Multi-factor rate models |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| PV / NPV | P1 | Present value |
| DV01 (parallel) | P1 | Dollar value of 01 |
| DV01 (bucketed / KRD) | P1 | Key rate duration |
| Gamma (parallel) | P1 | Second-order rate risk |
| Cross-gamma | P2 | Bucket cross-sensitivities |
| Theta / time decay | P1 | P&L from passage of time |
| Vega (flat) | P1 | Parallel vol sensitivity |
| Vega (bucketed) | P2 | By expiry and/or tenor |
| Convexity adjustment | P2 | Futures vs forwards |
| Par rate sensitivity | P2 | Sensitivity to par instruments |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| SOFR day count | ACT/360 | Daily compounding in-arrears |
| SOFR compounding | Compounded in-arrears | Model lookback, observation shift, lockout, and payment lag separately |
| SOFR payment lag | Template-specific, 2 business days common in USD OIS | Payment offset after period end |
| EURIBOR day count | ACT/360 | Term rate, no compounding |
| SONIA day count | ACT/365F | Daily compounding in-arrears |
| TONA day count | ACT/365F | Japan overnight rate |
| USD OIS fixed leg | Template-specific | Do not reuse legacy IRS conventions for SOFR OIS |
| EUR OIS fixed leg | Template-specific | Check CCP / venue template |
| GBP OIS fixed leg | Annual, ACT/365F | Standard SONIA OIS convention |
| Term-index IRS fixed leg | Product-specific | See `rates-standards.md` legacy / term-index section |
| Cap/floor settlement | Cash-settled caplets/floorlets | Premium timing per confirmation, not physical delivery |
| Swaption exercise | European: expiry, Bermudan: coupon dates | Physical or cash |
| IMM futures dates | 3rd Wed of Mar/Jun/Sep/Dec | Futures and many listed IR products |
| CDS standard dates | 20th of Mar/Jun/Sep/Dec | Standard CDS roll and maturity dates |
| Swap roll | Product / template specific | Vanilla term-index IRS often use modified following with EOM logic, but do not apply universally |

---

## Credit

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Single-name CDS | P1 | Standard (100/500 bps running) |
| CDS index (CDX/iTraxx) | P1 | On-the-run and off-the-run |
| CDS index option | P2 | Options on credit indices |
| CDS tranche | P2 | Synthetic CDO |
| CDS swaption | P3 | Options on single-name CDS |
| Loan CDS (LCDS) | P3 | Loan-linked credit default |
| Total return swap (credit) | P2 | Funded credit exposure |
| CLN (credit-linked note) | P3 | Structured credit note |
| Nth-to-default basket | P2 | Basket credit derivatives |

### Pricing Models

| Model | Priority | Instruments |
|-------|----------|-------------|
| ISDA Standard CDS model | P1 | Single-name CDS |
| Hazard rate bootstrap | P1 | Curve construction from CDS |
| Gaussian copula | P2 | CDO tranche pricing |
| Base correlation | P2 | Tranche interpolation |
| Homogeneous pool | P2 | Large portfolio approximation |
| Stochastic recovery | P3 | Recovery rate uncertainty |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| CS01 (parallel) | P1 | Credit spread 01 |
| CS01 (bucketed) | P1 | By tenor |
| Recovery01 | P1 | Recovery rate sensitivity |
| JTD (jump-to-default) | P1 | Instantaneous default loss |
| Spread gamma | P2 | Second-order spread risk |
| Default probability | P1 | Term structure |
| Survival probability | P1 | Term structure |
| Expected loss | P1 | PD x LGD |
| Risky PV01 | P1 | Credit-adjusted annuity |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| CDS standard coupon | 100bp (IG) or 500bp (HY) | North American convention |
| CDS accrual | ACT/360 | Standard |
| CDS payment frequency | Quarterly | On IMM dates |
| CDS step-in / protection effective date | T+1 | Contractual accrual start still follows standard CDS date rules |
| CDS roll dates | Mar/Jun/Sep/Dec 20th | Standard roll |
| Accrued-on-default | Paid by protection buyer | ISDA 2014 |
| Recovery rate (senior unsecured) | 40% | ISDA standard assumption |
| Recovery rate (subordinated) | 20% | ISDA standard assumption |
| Recovery rate (senior secured / LCDS) | Product-specific | Do not hardcode a universal secured-loan recovery assumption |
| CDX IG composition | 125 names | Investment grade |
| iTraxx Europe composition | 125 names | Investment grade |

---

## FX

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| FX spot | P1 | Spot position |
| FX forward / outright | P1 | T+n delivery |
| FX swap (near/far) | P1 | Spot + forward |
| NDF | P1 | Non-deliverable forward |
| FX option (vanilla) | P1 | European call/put |
| FX option (American) | P2 | Early exercise |
| FX barrier option | P1 | KI/KO single barrier |
| FX double barrier | P2 | Double knock-in/out |
| FX digital option | P2 | Binary payout |
| FX touch option | P2 | One-touch, no-touch |
| FX Asian option | P2 | Average rate option |
| FX basket option | P2 | Multi-pair basket |
| FX target accrual (TARF) | P3 | Path-dependent structured |
| FX variance swap | P2 | Realized vs implied variance |
| Cross-currency basis swap | P1 | Covered in rates section |

### Pricing Models

| Model | Priority | Instruments |
|-------|----------|-------------|
| Garman-Kohlhagen | P1 | Vanilla FX options |
| Vanna-Volga | P2 | Risk-neutral barrier pricing |
| Local volatility (Dupire) | P2 | Barrier, digital, exotic |
| Stochastic volatility (Heston) | P2 | Smile dynamics |
| SABR for FX | P2 | Vol surface interpolation |
| SLV (Stochastic Local Vol) | P3 | Best-practice exotic pricing |
| Monte Carlo (multi-currency) | P2 | Path-dependent exotics |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| Delta (spot) | P1 | Spot FX sensitivity |
| Delta (forward) | P1 | Forward FX sensitivity |
| Gamma | P1 | Second-order spot risk |
| Vega (flat) | P1 | Implied vol sensitivity |
| Vega (by expiry) | P2 | Bucketed by tenor |
| Vanna | P2 | dDelta/dVol |
| Volga | P2 | dVega/dVol |
| Theta | P1 | Time decay |
| Rho (domestic) | P1 | Domestic rate sensitivity |
| Rho (foreign) | P1 | Foreign rate sensitivity |
| FX01 | P1 | Spot bump sensitivity |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| Spot settlement | T+2 | Most G10 pairs |
| USD/CAD settlement | T+1 | Exception to T+2 |
| USD/TRY settlement | T+1 | Exception |
| USD/RUB settlement | T+1 | Exception |
| FX option delta | Pair-specific: forward delta common in G10, premium-adjusted for some EM | Never assume spot delta or forward delta without confirming pair and venue |
| FX option premium | Pair-specific | CCY2 common in G10, USD common in many EM pairs |
| Vol quote convention | RR/BF plus ATM convention | Pair-specific smile quoting template |
| ATM convention | Pair-specific | DNS common in some markets, but do not assume globally |
| Cut time | Pair-specific benchmark / cut | Use the benchmark named in the market convention and confirmation |
| FX forward points | Pair-specific pip scaling | 1/10000 for most pairs, 1/100 for JPY pairs |
| Quanto correlation | Spot-FX correlation | Must be estimated, not observed |

---

## Fixed Income (Cash Bonds)

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Government bond (fixed) | P1 | Treasury, Bund, Gilt, JGB |
| Corporate bond (fixed) | P1 | Investment grade and HY |
| Floating rate note (FRN) | P1 | SOFR/EURIBOR linked |
| Inflation-linked bond | P1 | TIPS, linkers |
| Callable bond | P1 | Issuer call schedule |
| Putable bond | P2 | Holder put schedule |
| Convertible bond | P2 | Equity conversion feature |
| Amortizing bond | P2 | Sinking fund, scheduled amort |
| Zero-coupon bond | P1 | Discount instrument |
| Perpetual bond | P2 | No maturity date |
| PIK bond | P2 | Payment-in-kind, capitalize interest |
| Step-up bond | P2 | Coupon increases per schedule |
| Sukuk | P3 | Islamic finance |
| Covered bond | P2 | Pfandbrief-style |
| Municipal bond | P2 | Tax-advantaged |

### Pricing Approaches

| Method | Priority | Notes |
|--------|----------|-------|
| Yield-to-maturity | P1 | Flat curve pricing |
| Z-spread | P1 | Parallel shift to risk-free |
| I-spread | P2 | Spread to swap curve |
| OAS | P1 | Option-adjusted spread (requires model for callables) |
| ASW (asset swap spread) | P2 | Relative value metric |
| Discount margin (FRN) | P1 | FRN-specific spread |
| CDS-bond basis | P2 | Credit relative value |
| Make-whole price | P2 | PV of remaining flows at T+spread for callable |
| Yield-to-worst | P1 | Min yield across call dates |
| Yield-to-call | P1 | Yield assuming called at each date |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| Clean / dirty price | P1 | Settlement convention |
| Accrued interest | P1 | Per day count convention |
| YTM | P1 | Internal rate of return |
| Modified duration | P1 | Price sensitivity to yield |
| Macaulay duration | P1 | Weighted average life |
| Effective duration | P1 | For bonds with optionality |
| Convexity | P1 | Second-order yield risk |
| Effective convexity | P1 | For callable bonds |
| DV01 | P1 | Dollar value of a basis point |
| Key rate duration | P1 | Bucketed yield sensitivity |
| OAS duration | P2 | Duration at constant OAS |
| Spread duration | P1 | Sensitivity to credit spread |
| Z-spread sensitivity | P2 | dP/dZ |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| UST day count | ACT/ACT (ICMA / Street) | Semi-annual coupons |
| UST settlement | T+1 | Changed from T+2 in May 2023 |
| Bund day count | ACT/ACT (ICMA) | Annual coupons |
| Gilt day count | ACT/ACT (ICMA) | Semi-annual, 7-day ex-div |
| JGB day count | varies | Simple yield convention |
| Corporate bond day count | 30/360 (US), ACT/ACT (EUR) | Market dependent |
| Corporate settlement | T+1 in current US cash market | Check market and settlement regime outside the US |
| Accrued interest (UST) | ACT/ACT, inclusive start, exclusive end | |
| Price quote | Per 100 face value | Clean price |
| YTM compounding | Semi-annual (US), Annual (EUR) | Market dependent |
| TIPS indexation | CPI-U with 3-month lag | Interpolated daily |
| Muni tax-equivalent yield | Adjust for tax bracket | Federal and state |

---

## Equity & Equity Derivatives

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Equity spot | P1 | Single stock / index |
| Equity forward | P1 | Forward on stock/index |
| Equity option (European) | P1 | Vanilla call/put |
| Equity option (American) | P1 | Early exercise |
| Equity index future | P1 | Exchange-traded |
| Equity index option | P1 | Exchange-traded |
| Dividend swap | P2 | Implied vs realized dividends |
| Total return swap | P1 | Equity TRS |
| Variance swap | P1 | Realized vs implied variance |
| Volatility swap | P2 | Realized vs implied vol |
| Equity barrier option | P2 | Knock-in/out |
| Equity Asian option | P2 | Average price/strike |
| Equity basket option | P2 | Worst-of, best-of, rainbow |
| Autocallable | P2 | Structured product |
| Cliquet / ratchet | P2 | Forward-starting options |
| Lookback option | P3 | Floating/fixed strike |
| Quanto option | P2 | Cross-currency equity |

### Pricing Models

| Model | Priority | Instruments |
|-------|----------|-------------|
| Black-Scholes-Merton | P1 | European vanilla |
| Binomial tree (CRR) | P1 | American vanilla |
| Heston | P1 | Stochastic vol surfaces |
| Local volatility (Dupire) | P2 | Barrier, digital |
| Bates (Heston + jumps) | P3 | Jump-diffusion |
| Longstaff-Schwartz (LSM) | P1 | American/Bermudan MC |
| Variance swap replication | P2 | Log-contract based |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| Delta | P1 | |
| Gamma | P1 | |
| Vega | P1 | |
| Theta | P1 | |
| Rho | P1 | |
| Vanna | P2 | dDelta/dVol |
| Volga | P2 | dVega/dVol |
| Charm | P3 | dDelta/dTime |
| Speed | P3 | dGamma/dSpot |
| Color | P3 | dGamma/dTime |
| Implied volatility | P1 | Newton-Raphson / Jaeckel |
| Variance vega | P1 | For variance swaps |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| Equity option expiry | 3rd Friday of month (US listed) | OTC negotiated |
| Dividend handling | Discrete dividends with ex-date adjustment | Use discounted dividends or explicit ex-date modeling, not raw spot subtraction |
| Borrow cost | Subtracted from drift | For short selling |
| Variance swap convention | Realized var = (252/N) * sum(ln(S_i/S_{i-1})^2) | Annualized, 252 trading days |
| Vol swap vs var swap | Vol swap != sqrt(var swap) | Jensen's inequality |
| TRS financing | Funding index of trade currency + spread | SOFR/SONIA/ESTR etc., reset per contract |
| Index dividend yield | Continuous vs discrete matters | Affects forward and options |
| Autocallable observation | Closing price on observation dates | Typically monthly or quarterly |

---

## Commodities

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Commodity spot | P1 | Physical position |
| Commodity forward | P1 | Physical or financial settlement |
| Commodity future | P1 | Exchange-traded (CME, ICE, LME) |
| Commodity swap | P1 | Fixed vs floating (e.g., oil swap) |
| Commodity option (European) | P1 | Options on futures |
| Commodity option (American) | P2 | Physical delivery options |
| Commodity Asian option | P1 | Average price (very common in commodities) |
| Commodity spread option | P2 | Crack, spark, crush spreads |
| Commodity swaption | P3 | Options on commodity swaps |
| Commodity calendar spread | P2 | Roll/contango/backwardation trading |
| Commodity basis swap | P2 | Location or quality basis |

### Pricing Models

| Model | Priority | Instruments |
|-------|----------|-------------|
| Cost-of-carry / forward pricing | P1 | Forwards, futures |
| Black-76 | P1 | European options on futures |
| Schwartz 1F (mean-reverting) | P2 | Commodity-specific dynamics |
| Schwartz-Smith 2F | P2 | Short-term/long-term decomposition |
| Kirk approximation | P2 | Spread options |
| Gabillon | P3 | Forward curve dynamics |
| Monte Carlo (multi-factor) | P2 | Path-dependent exotics |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| Delta (spot and forward) | P1 | Price sensitivity |
| Gamma | P1 | Convexity |
| Vega | P1 | Vol sensitivity |
| Theta | P1 | Time decay |
| Bucket delta | P2 | By delivery period |
| Roll risk | P2 | Sensitivity to term structure shape |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| Oil forward curve | Monthly contract expiry | WTI: 3 business days before 25th |
| Asian averaging | Arithmetic average of daily settlement | Most common in energy |
| Commodity day count | ACT/365F or ACT/360 | Market dependent |
| LME settlement | Prompt dates (3M, 15M, 27M) | Not standard monthly |
| Commodity seasonality | Vol and price seasonal patterns | Natural gas, agriculture |
| Convenience yield | Implied from futures curve | Backwardation indicator |
| Contango/backwardation | Forward curve shape | Affects roll return |

---

## Leveraged Finance & Private Credit

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Term loan A (TLA) | P1 | Amortizing, drawn at close |
| Term loan B (TLB) | P1 | Bullet, institutional tranche |
| Second lien term loan | P2 | Junior secured |
| Unitranche | P2 | Blended first/second lien |
| Revolving credit facility | P1 | Drawn/undrawn commitment |
| Delayed-draw term loan (DDTL) | P2 | Future funding commitment |
| Bridge loan | P2 | Short-term acquisition financing |
| PIK / toggle note | P2 | Capitalize interest, optional cash pay |
| Mezzanine loan | P2 | Subordinated, often with equity kicker |
| Direct lending facility | P2 | Private credit, unitranche structure |
| First lien / last out | P3 | Split-lien structure |

### Pricing Features

| Feature | Priority | Notes |
|---------|----------|-------|
| SOFR + spread pricing | P1 | Floating rate with floor |
| SOFR floor | P1 | Typically 0% or 1% floor |
| OID (original issue discount) | P1 | Day-1 economics, amortized |
| Call protection | P1 | Non-call period, soft call premium |
| Make-whole provision | P2 | Call at PV of remaining flows at T+50bp |
| Repricing mechanics | P2 | Spread tightening without refi |
| Prepayment penalty | P1 | Typically 101/par schedule |
| PIK capitalization | P2 | Compound notional, timing convention |
| Commitment fee | P1 | Fee on undrawn revolver (typically 25-50bp) |
| Ticking fee | P2 | Fee during commitment period before funding |
| Utilization fee | P2 | Additional spread when drawn above threshold |

### Risk Metrics

| Metric | Priority | Notes |
|--------|----------|-------|
| Spread duration | P1 | Sensitivity to credit spread |
| DM01 (discount margin 01) | P1 | 1bp change in discount margin |
| WAL (weighted average life) | P1 | With prepayment assumptions |
| Yield to maturity | P1 | Including OID amortization |
| Expected loss | P1 | PD x LGD based on leverage |
| Recovery analysis | P1 | Waterfall based on EV |
| Drawn/undrawn exposure | P1 | For revolvers, usage-based |
| Commitment value | P1 | Undrawn commitment PV |

### Covenant Testing

| Feature | Priority | Notes |
|---------|----------|-------|
| Leverage ratio test | P1 | Total debt / EBITDA |
| Interest coverage ratio | P1 | EBITDA / interest expense |
| Fixed charge coverage | P2 | (EBITDA - capex) / fixed charges |
| Minimum liquidity | P2 | Cash + undrawn revolver |
| Maximum capex | P2 | Annual capital expenditure limit |
| Excess cash flow sweep | P1 | Mandatory prepayment from cash flow |
| Asset sale sweep | P2 | Mandatory prepayment from dispositions |
| EBITDA adjustments | P1 | Add-backs, run-rate adjustments |
| Covenant-lite (cov-lite) | P1 | Incurrence-based only, no maintenance |
| Springing covenant | P2 | Activates when revolver drawn >35% |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| Loan day count | ACT/360 | Standard for USD leveraged loans |
| Payment frequency | Quarterly | Interest and amortization |
| SOFR coupon convention | Agreement-specific | Daily simple SOFR loans often use longer lookbacks; term SOFR loans use term fixings |
| SOFR floor | 0% minimum | Some deals have 50-100bp floor |
| Amortization (TLA) | 1% per quarter typical | Varies by deal |
| Amortization (TLB) | 1% per annum | Minimal, bullet at maturity |
| Prepayment notice | 1-3 business days | Per credit agreement |
| OID accounting | Amortized over expected life | Not stated maturity |
| Excess cash flow calc | Annual, 75-day test period | Based on fiscal year |

---

## Money Markets

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Treasury bill | P1 | Discount instrument, 4/8/13/26/52 week |
| Commercial paper | P2 | Corporate short-term, up to 270 days |
| Certificate of deposit (CD) | P2 | Bank deposit, fixed term |
| Bankers' acceptance | P3 | Trade finance instrument |
| Fed funds (overnight) | P1 | Interbank overnight lending |
| Term deposit | P1 | Fixed-rate bank deposit |
| Money market fund | P3 | NAV-based, not a single instrument |

### Pricing Features

| Feature | Priority | Notes |
|---------|----------|-------|
| Discount yield | P1 | T-bill convention: (Face-Price)/Face * 360/days |
| Bond-equivalent yield | P1 | Convert discount to BEY for comparison |
| Money market yield | P1 | ACT/360 simple interest |
| Add-on yield | P1 | CD convention |
| Holding period return | P1 | For short-term investment analysis |

### Convention Compliance

| Convention | Standard | Notes |
|-----------|----------|-------|
| T-bill day count | ACT/360 (discount) | Bank discount basis |
| T-bill settlement | T+1 | Same as UST since 2023 |
| T-bill pricing | Per $100 face, discount | Price = 100 × (1 - discount × days/360) |
| CP day count | ACT/360 | Discount basis |
| CD day count | ACT/360 | Add-on basis |
| Fed funds day count | ACT/360 | Overnight rate |

---

## Structured Products & Securitization

### Instruments

| Instrument | Priority | Notes |
|------------|----------|-------|
| Agency MBS pass-through | P2 | Prepayment modeling |
| Agency CMO | P2 | Tranched MBS |
| TBA | P2 | To-be-announced |
| Non-agency RMBS | P3 | Private-label |
| CMBS | P3 | Commercial MBS |
| ABS (auto, card, student) | P3 | Consumer ABS |
| CLO | P2 | Leveraged loan securitization |
| Synthetic CDO | P2 | Credit tranche via CDS |

### Key Features

| Feature | Priority | Notes |
|---------|----------|-------|
| Prepayment model (PSA/CPR) | P2 | MBS cashflow projection |
| Default/loss model | P2 | CDR, severity, timing |
| Waterfall engine | P2 | Cash allocation rules |
| OAS analytics | P2 | Option-adjusted spread |
| Effective duration/convexity | P2 | For MBS |
| WAL (weighted avg life) | P2 | Amortization metric |

### CLO-Specific Features

| Feature | Priority | Notes |
|---------|----------|-------|
| CLO waterfall | P2 | Par/interest waterfall with coverage tests |
| Overcollateralization test | P2 | Par value / tranche notional |
| Interest coverage test | P2 | Interest received / interest due |
| Reinvestment period | P2 | Period during which proceeds are reinvested |
| Non-call period | P2 | Typically 2 years |
| Refinancing mechanics | P2 | Re-price or reset |
| WARF (weighted avg rating factor) | P2 | Portfolio quality metric |
| Diversity score | P2 | Moody's industry concentration |
| CCC bucket limit | P2 | Typically 7.5% of portfolio |
| Discount obligation purchase | P3 | Trading discount loans below par |

---

## Cross-Cutting Concerns

### Market Data & Curves

| Feature | Priority | Notes |
|---------|----------|-------|
| Multi-curve framework | P1 | Separate discount/projection |
| OIS discounting | P1 | Post-crisis standard |
| CSA-aware discounting | P1 | Collateral currency |
| Cross-currency basis | P1 | FX basis adjustment |
| Dual-curve stripping | P1 | Simultaneous OIS + IBOR |
| Cubic spline interpolation | P1 | Smooth curves |
| Log-linear on discount factors | P1 | Standard bootstrapping |
| Monotone convex | P2 | Hagan-West method |
| Tension spline | P3 | Alternative smoothing |
| Turn-of-year effects | P2 | Short-end seasonality |
| Fixing history | P1 | Historical rate fixings |
| Implied forward rates | P1 | From the relevant projection curve, not the discount curve |
| Basis-adjusted forwards | P1 | Tenor basis in projections |
| Central bank meeting date bumps | P2 | Rate step adjustments at FOMC/ECB dates |
| Negative rate handling | P1 | DF > 1 can be valid in negative-rate markets; handle interpolation and quoting consistently |
| Curve snapshot/restore | P1 | Save/load calibrated state |
| Curve diffing | P2 | Compare two snapshots for P&L attribution |

### Vol Surfaces

| Feature | Priority | Notes |
|---------|----------|-------|
| Strike-by-expiry grid | P1 | Standard surface |
| SABR calibration | P1 | Smile interpolation |
| SVI parameterization | P2 | Equity vol surfaces |
| Sticky strike / sticky delta | P2 | Surface dynamics |
| Vol smile extrapolation | P2 | Wing behavior |
| Swaption vol cube | P2 | Expiry x tenor x strike |
| FX vol surface (delta-space) | P1 | 25D, 10D RR/BF quotes |
| Local vol extraction | P2 | Dupire from implied vol |
| Forward vol | P2 | Vol between future dates |
| Vol surface arbitrage checks | P2 | Calendar spread, butterfly |
| Normal vs lognormal conversion | P1 | Bachelier <-> Black |
| ATM vol extraction | P1 | From surface for Greeks |

### Risk & Scenario Framework

| Feature | Priority | Notes |
|---------|----------|-------|
| Parallel bump | P1 | All risk factors |
| Bucketed bump | P1 | Individual tenor/strike |
| Cross-gamma | P2 | Between different curves |
| Scenario analysis | P1 | Custom shock sets |
| Historical VaR | P1 | Full revaluation |
| Parametric VaR | P2 | Variance-covariance |
| Expected Shortfall (CVaR) | P1 | Tail risk |
| Stress testing | P1 | Predefined scenarios |
| P&L attribution | P2 | By risk factor |
| P&L explain | P2 | Actual vs theoretical |
| Taylor expansion VaR | P2 | Delta-gamma approximation |
| Incremental VaR | P3 | Marginal contribution |
| What-if analysis | P2 | Impact of adding/removing trades |
| Regulatory stress scenarios | P2 | CCAR, DFAST, EBA |

### Calibration

| Feature | Priority | Notes |
|---------|----------|-------|
| Curve bootstrap (exact fit) | P1 | Sequential instrument stripping |
| Global optimization (best fit) | P2 | Levenberg-Marquardt |
| SABR calibration | P1 | Alpha, rho, nu from market |
| Heston calibration | P2 | From option surface |
| HW1F calibration | P2 | From swaptions or caps |
| Jacobian / instrument sensitivity | P2 | Curve risk decomposition |
| Calibration diagnostics | P1 | Residuals, convergence, warnings |
| Calibration audit trail | P1 | Reproducibility for regulatory |
| Fallback chain | P2 | Try bootstrap, fallback to global if singular |
| Input validation | P1 | Reject stale/invalid market data |

### XVA (Valuation Adjustments)

| Feature | Priority | Notes |
|---------|----------|-------|
| CVA (credit valuation adjustment) | P1 | Counterparty credit risk |
| DVA (debit valuation adjustment) | P2 | Own credit |
| FVA (funding valuation adjustment) | P2 | Funding cost |
| ColVA (collateral valuation adj) | P3 | Collateral optionality |
| KVA (capital valuation adjustment) | P3 | Cost of capital |
| MVA (margin valuation adjustment) | P3 | Cost of initial margin |
| Wrong-way risk | P2 | Correlation CPD-exposure |
| Exposure simulation (EPE/ENE) | P1 | For CVA computation |
| Netting set modeling | P1 | CSA/ISDA netting |

### Margin & Regulatory

| Feature | Priority | Notes |
|---------|----------|-------|
| ISDA SIMM | P1 | Standardized IM model |
| CCP initial margin | P1 | Clearing house IM |
| Variation margin | P1 | Daily mark-to-market |
| SA-CCR | P2 | Standardized counterparty credit risk |
| Basel III capital | P3 | Regulatory capital |
| FRTB (IMA/SA) | P3 | Market risk capital |

### Portfolio & Aggregation

| Feature | Priority | Notes |
|---------|----------|-------|
| Multi-currency aggregation | P1 | FX conversion |
| Netting | P1 | Legal netting sets |
| Attribute-based grouping | P1 | By desk, book, counterparty |
| Batch pricing | P1 | Portfolio-level valuation |
| Parallel computation | P2 | Multi-threaded pricing |
| Incremental risk | P2 | What-if analysis |
| Trade lifecycle | P2 | Novation, termination, amendment |

### P&L Attribution

| Feature | Priority | Notes |
|---------|----------|-------|
| Delta P&L | P1 | First-order market move contribution |
| Gamma P&L | P1 | Second-order market move contribution |
| Vega P&L | P1 | Volatility change contribution |
| Theta P&L | P1 | Time decay, carry, roll-down |
| Cross-effect P&L | P2 | Interaction terms |
| New deal P&L | P1 | Day-1 P&L from new trades |
| Full reval P&L | P1 | Exact P&L from full revaluation |
| Unexplained residual | P1 | Full reval - sum(explained), should be < 5% |
| P&L by risk factor | P2 | Which curve/surface drove the P&L |
| P&L by trade | P1 | Which trade drove the P&L |
| Carry decomposition | P2 | Coupon income, funding cost, roll-down |
| Market data diff | P1 | Compute risk factor moves between dates |

### Trade Lifecycle

| Feature | Priority | Notes |
|---------|----------|-------|
| Rate fixing/reset | P1 | Historical fixings for floating legs |
| Inflation fixing | P1 | CPI index lookup with publication lag |
| FX fixing | P1 | Pair-specific benchmark from market convention and confirmation |
| Exercise handling | P1 | European auto-exercise, Bermudan/American decisions |
| Barrier monitoring | P1 | Continuous or discrete observation |
| Coupon payment | P1 | Calculate amount, adjust for business day |
| Amortization event | P1 | Reduce notional per schedule |
| Prepayment | P2 | MBS/loan prepayment modeling |
| Credit event | P2 | Default, restructuring, succession |
| Call/put exercise | P1 | For callable/putable bonds |
| Make-whole calculation | P2 | PV at T+spread for make-whole calls |
| Novation/assignment | P2 | Transfer of trade to new counterparty |
| Dividend adjustment | P1 | Equity options, TRS, convertibles |
| Margin call | P1 | VM and IM calculation |
| PIK toggle | P2 | Cash vs in-kind payment decision |

### Numerical Methods

| Feature | Priority | Notes |
|---------|----------|-------|
| Newton-Raphson with fallback | P1 | Brent/bisection when Newton diverges |
| Implied vol solver | P1 | Jaeckel (2017) rational approximation preferred |
| Quasi-random MC (Sobol) | P1 | Low-discrepancy sequences |
| Antithetic variates | P1 | Basic variance reduction |
| Control variates | P2 | European price as control |
| Importance sampling | P3 | For tail risk / rare events |
| Kahan summation | P1 | Stable summation for large cashflow sets |
| Adaptive time stepping | P2 | Finer steps near barriers/exercise dates |
| Richardson extrapolation | P2 | Improve tree/PDE convergence |
| Cholesky decomposition | P1 | Correlated multi-asset simulation |
| Central differencing for Greeks | P1 | O(h^2) vs O(h) for forward |
| Bump size configuration | P1 | Must be adjustable per risk factor |
| Convergence monitoring | P1 | MC standard error, tree step sensitivity |
| Calibration diagnostics | P1 | Residuals, Jacobian condition, convergence flag |

### Data Quality & Market Data Handling

| Feature | Priority | Notes |
|---------|----------|-------|
| Missing data handling | P1 | Interpolation or last-known-good |
| Stale data detection | P1 | Flag quotes older than threshold |
| Quote validation | P1 | Reject negative vol, inverted spreads |
| Bid/ask handling | P2 | Mid, bid, ask, or custom weighting |
| Snap time management | P2 | EOD vs intraday, timezone-aware |
| Holiday-adjusted fixing | P1 | Follow the product-specific fixing convention, not a universal previous-business-day rule |
| Data source priority | P2 | Primary/secondary/fallback sources |
| Audit trail | P1 | Which data was used for each calculation |
