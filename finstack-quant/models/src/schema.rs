//! Schema registry for the models crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_models_schemas` binary writes one JSON Schema per
//! entry, and the WASM package generates its TypeScript declarations from
//! them. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::schema::SchemaArtifact;

use crate::{
    correlation::CreditExposure, correlation::LatentFactorSpec, correlation::PortfolioLossConfig,
    correlation::PortfolioLossResult, correlation::RecoverySpec,
    correlation::TrancheLossStatistics, credit::lgd::DownturnLgd, credit::lgd::EadCalculator,
    credit::lgd::SeniorityCalibration, credit::lgd::SeniorityRecovery, credit::lgd::WorkoutLgd,
    credit::lgd::WorkoutLgdResult, credit::liability_management::ExchangeOfferAnalysis,
    credit::liability_management::LmeAnalysis, credit::migration::MigrationSimulator,
    credit::migration::RatingPath, credit::migration::TransitionMatrix, credit::pd::MasterScale,
    credit::pd::MasterScaleResult, credit::pd::PdCycleParams, credit::pool::CorrelationStructure,
    credit::pool::PoolGranularity, credit::pool::StochasticDefaultSpec,
    credit::pool::StochasticPrepaySpec, credit::recovery_waterfall::RecoveryClaim,
    credit::recovery_waterfall::RecoveryWaterfallResult, credit::scoring::AltmanZDoublePrimeInput,
    credit::scoring::AltmanZPrimeInput, credit::scoring::AltmanZScoreInput,
    credit::scoring::OhlsonOScoreInput, credit::scoring::ScoringResult,
    credit::scoring::ZmijewskiInput, credit::CreditState, credit::DynamicRecoverySpec,
    credit::EndogenousHazardSpec, credit::MertonModel, credit::RatingFactorTable,
    credit::SimulatedPaths, credit::ToggleExerciseModel,
    factor::credit::decomposition::LevelsAtDate,
    factor::credit::decomposition::PeriodDecomposition, factor::risk::DecompositionConfig,
    factor::risk::ParametricEsDecompositionView, factor::risk::PositionRiskDecomposition,
    factor::risk::RiskBudgetResult, factor::risk::RiskDecomposition,
    factor::risk::StressAttribution, factor::MarketDependency,
    fourier::characteristic_function::BlackScholesCf,
    fourier::characteristic_function::MertonJumpCf,
    fourier::characteristic_function::VarianceGammaCf, fourier::CosConfig,
    liquidity::AlmgrenChrissModel, liquidity::ExecutionTrajectory, liquidity::ImpactEstimate,
    liquidity::KyleLambdaModel, liquidity::LiquidityConfig, liquidity::LiquidityTier,
    liquidity::LvarBangiaScalar, liquidity::TradeParams, monte_carlo::estimate::Estimate,
    monte_carlo::payoff::asian::AveragingMethod, monte_carlo::payoff::barrier::OptionKind,
    monte_carlo::pricer::basis::BasisKind,
    monte_carlo::process::cheyette_rough::CheyetteRoughVolParams,
    monte_carlo::process::cir::CirParams, monte_carlo::process::lmm::LmmParams,
    monte_carlo::process::ou::HullWhite1FParams,
    monte_carlo::process::rough_bergomi::RoughBergomiParams,
    monte_carlo::process::rough_heston::RoughHestonParams,
    monte_carlo::process::schwartz_smith::SchwartzSmithParams,
    monte_carlo::process::BrownianParams, monte_carlo::process::GbmParams,
    monte_carlo::process::MultiOuParams, monte_carlo::results::MonteCarloResult,
    monte_carlo::GbmPathSummary, rates::dtsm::DieboldLi, rates::dtsm::YieldForecast,
    rates::dtsm::YieldPanel, rates::dtsm::YieldPca, rates::dtsm::YieldPcaView,
    rates::hull_white::HullWhiteCalibrationParams, rates::hull_white::HullWhiteParams,
    volatility::arbitrage::ArbitrageCheckConfig, volatility::arbitrage::ArbitrageReport,
    volatility::heston::HestonParams, volatility::rough_heston::RoughHestonFourierParams,
    volatility::sabr::ArbitrageValidationResult, volatility::svi::SviParams,
    volatility::SabrMarketData, volatility::VolatilityConvention, BsGreeks, ExerciseStyle,
    ForwardGreeks, HestonPricingParams, OptionType, SabrParameters,
};

/// The crate's complete schema registry.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        AlmgrenChrissModel,
        "models",
        "almgren_chriss_model",
        Input,
        "Almgren-Chriss (2001) market impact model."
    ),
    finstack_quant_core::schema_artifact!(
        AltmanZDoublePrimeInput,
        "models",
        "altman_z_double_prime_input",
        Input,
        "Input ratios for the Altman Z''-Score (non-manufacturing firms)."
    ),
    finstack_quant_core::schema_artifact!(
        AltmanZPrimeInput,
        "models",
        "altman_z_prime_input",
        Input,
        "Input ratios for the Altman Z'-Score (private firms)."
    ),
    finstack_quant_core::schema_artifact!(
        AltmanZScoreInput,
        "models",
        "altman_z_score_input",
        Input,
        "Input ratios for the original Altman Z-Score (1968)."
    ),
    finstack_quant_core::schema_artifact!(
        ArbitrageCheckConfig,
        "models",
        "arbitrage_check_config",
        Input,
        "Configuration for the arbitrage detection suite."
    ),
    finstack_quant_core::schema_artifact!(
        ArbitrageReport,
        "models",
        "arbitrage_report",
        Output,
        "Aggregated arbitrage report for a volatility surface."
    ),
    finstack_quant_core::schema_artifact!(
        ArbitrageValidationResult,
        "models",
        "arbitrage_validation_result",
        Output,
        "Result of arbitrage validation, containing any violations found."
    ),
    finstack_quant_core::schema_artifact!(
        AveragingMethod,
        "models",
        "averaging_method",
        Component,
        "Asian averaging method."
    ),
    finstack_quant_core::schema_artifact!(
        BasisKind,
        "models",
        "basis_kind",
        Component,
        "Supported regression basis families for LSMC pricers."
    ),
    finstack_quant_core::schema_artifact!(
        BlackScholesCf,
        "models",
        "black_scholes_cf",
        Component,
        "Black-Scholes characteristic function."
    ),
    finstack_quant_core::schema_artifact!(
        BrownianParams,
        "models",
        "brownian_params",
        Input,
        "Parameters for one-dimensional Brownian motion with drift."
    ),
    finstack_quant_core::schema_artifact!(
        BsGreeks,
        "models",
        "bs_greeks",
        Component,
        "Black–Scholes/Garman–Kohlhagen Greeks (per unit, not scaled by contract size)."
    ),
    finstack_quant_core::schema_artifact!(
        CheyetteRoughVolParams,
        "models",
        "cheyette_rough_vol_params",
        Input,
        "Cheyette + rough stochastic volatility model parameters."
    ),
    finstack_quant_core::schema_artifact!(
        CirParams,
        "models",
        "cir_params",
        Input,
        "CIR process parameters."
    ),
    finstack_quant_core::schema_artifact!(
        CorrelationStructure,
        "models",
        "correlation_structure",
        Component,
        "Correlation structure specification."
    ),
    finstack_quant_core::schema_artifact!(
        CosConfig,
        "models",
        "cos_config",
        Input,
        "COS method configuration."
    ),
    finstack_quant_core::schema_artifact!(
        CreditExposure,
        "models",
        "credit_exposure",
        Component,
        "One name in a finite credit portfolio."
    ),
    finstack_quant_core::schema_artifact!(
        CreditState,
        "models",
        "credit_state",
        Component,
        "Observable credit state at a point in time."
    ),
    finstack_quant_core::schema_artifact!(
        DecompositionConfig,
        "models",
        "decomposition_config",
        Input,
        "Configuration for position-level VaR decomposition."
    ),
    finstack_quant_core::schema_artifact!(
        DieboldLi,
        "models",
        "diebold_li",
        Component,
        "Diebold-Li (2006) dynamic Nelson-Siegel model."
    ),
    finstack_quant_core::schema_artifact!(
        DownturnLgd,
        "models",
        "downturn_lgd",
        Component,
        "Downturn LGD adjuster."
    ),
    finstack_quant_core::schema_artifact!(
        DynamicRecoverySpec,
        "models",
        "dynamic_recovery_spec",
        Input,
        "Specification for dynamic (notional-dependent) recovery rate."
    ),
    finstack_quant_core::schema_artifact!(
        EadCalculator,
        "models",
        "ead_calculator",
        Component,
        "Exposure at Default calculator."
    ),
    finstack_quant_core::schema_artifact!(
        EndogenousHazardSpec,
        "models",
        "endogenous_hazard_spec",
        Input,
        "Specification for endogenous (leverage-dependent) hazard rate."
    ),
    finstack_quant_core::schema_artifact!(
        Estimate,
        "models",
        "estimate",
        Output,
        "Numeric Monte Carlo estimate for discounted path values."
    ),
    finstack_quant_core::schema_artifact!(
        ExchangeOfferAnalysis,
        "models",
        "exchange_offer_analysis",
        Output,
        "Hold-versus-tender economics of a distressed exchange offer."
    ),
    finstack_quant_core::schema_artifact!(
        ExecutionTrajectory,
        "models",
        "execution_trajectory",
        Component,
        "Optimal execution schedule for a trade."
    ),
    finstack_quant_core::schema_artifact!(
        ExerciseStyle,
        "models",
        "exercise_style",
        Component,
        "Exercise schedule convention for option models."
    ),
    finstack_quant_core::schema_artifact!(
        ForwardGreeks,
        "models",
        "forward_greeks",
        Component,
        "Undiscounted forward Greeks of a European option on a forward."
    ),
    finstack_quant_core::schema_artifact!(
        GbmParams,
        "models",
        "gbm_params",
        Input,
        "Geometric Brownian Motion parameters."
    ),
    finstack_quant_core::schema_artifact!(
        GbmPathSummary,
        "models",
        "gbm_path_summary",
        Output,
        "Compact captured GBM paths for plotting and diagnostics."
    ),
    finstack_quant_core::schema_artifact!(
        HestonParams,
        "models",
        "heston_params",
        Input,
        "Heston stochastic volatility model parameters."
    ),
    finstack_quant_core::schema_artifact!(
        HestonPricingParams,
        "models",
        "heston_pricing_params",
        Input,
        "Market inputs for closed-form Heston pricing."
    ),
    finstack_quant_core::schema_artifact!(
        HullWhite1FParams,
        "models",
        "hull_white1_f_params",
        Input,
        "Validated Hull-White one-factor process parameters."
    ),
    finstack_quant_core::schema_artifact!(
        HullWhiteCalibrationParams,
        "models",
        "hull_white_calibration_params",
        Input,
        "Validated constant-parameter Hull-White one-factor model."
    ),
    finstack_quant_core::schema_artifact!(
        HullWhiteParams,
        "models",
        "hull_white_params",
        Input,
        "Hull-White parameters with piecewise-constant short-rate volatility."
    ),
    finstack_quant_core::schema_artifact!(
        ImpactEstimate,
        "models",
        "impact_estimate",
        Output,
        "Estimated market-impact execution *costs* from a trade."
    ),
    finstack_quant_core::schema_artifact!(
        KyleLambdaModel,
        "models",
        "kyle_lambda_model",
        Input,
        "Kyle (1985) price impact model."
    ),
    finstack_quant_core::schema_artifact!(
        LatentFactorSpec,
        "models",
        "latent_factor_spec",
        Input,
        "Factor model specification for configuration and serialization."
    ),
    finstack_quant_core::schema_artifact!(
        LevelsAtDate,
        "models",
        "levels_at_date",
        Component,
        "Snapshot of all hierarchy-level factor values at a single date, produced from observed issuer spreads."
    ),
    finstack_quant_core::schema_artifact!(
        LiquidityConfig,
        "models",
        "liquidity_config",
        Input,
        "Configuration for liquidity calculations."
    ),
    finstack_quant_core::schema_artifact!(
        LiquidityTier,
        "models",
        "liquidity_tier",
        Component,
        "Liquidity tier classification based on days-to-liquidate."
    ),
    finstack_quant_core::schema_artifact!(
        LmeAnalysis,
        "models",
        "lme_analysis",
        Output,
        "Issuer-side economics of a liability management exercise."
    ),
    finstack_quant_core::schema_artifact!(
        LmmParams,
        "models",
        "lmm_params",
        Input,
        "Parameters for the LMM/BGM model."
    ),
    finstack_quant_core::schema_artifact!(
        LvarBangiaScalar,
        "models",
        "lvar_bangia_scalar",
        Component,
        "Scalar Bangia LVaR outputs for an isolated position where relative spread statistics are already known."
    ),
    finstack_quant_core::schema_artifact!(
        MarketDependency,
        "models",
        "market_dependency",
        Component,
        "A single market dependency extracted from an instrument."
    ),
    finstack_quant_core::schema_artifact!(
        MasterScale,
        "models",
        "master_scale",
        Component,
        "A master scale mapping continuous PDs to discrete rating grades."
    ),
    finstack_quant_core::schema_artifact!(
        MasterScaleResult,
        "models",
        "master_scale_result",
        Output,
        "Result of mapping a PD to a master scale grade."
    ),
    finstack_quant_core::schema_artifact!(
        MertonJumpCf,
        "models",
        "merton_jump_cf",
        Component,
        "Merton (1976) jump-diffusion characteristic function."
    ),
    finstack_quant_core::schema_artifact!(
        MertonModel,
        "models",
        "merton_model",
        Input,
        "Merton structural credit model."
    ),
    finstack_quant_core::schema_artifact!(
        MigrationSimulator,
        "models",
        "migration_simulator",
        Component,
        "Simulator for generating rating paths from a generator matrix."
    ),
    finstack_quant_core::schema_artifact!(
        MonteCarloResult,
        "models",
        "monte_carlo_result",
        Output,
        "Monte Carlo pricing result with optional captured paths."
    ),
    finstack_quant_core::schema_artifact!(
        MultiOuParams,
        "models",
        "multi_ou_params",
        Input,
        "Parameters for a multi-dimensional Ornstein-Uhlenbeck process."
    ),
    finstack_quant_core::schema_artifact!(
        OhlsonOScoreInput,
        "models",
        "ohlson_o_score_input",
        Input,
        "Input for the Ohlson O-Score logistic model (1980)."
    ),
    finstack_quant_core::schema_artifact!(
        OptionKind,
        "models",
        "option_kind",
        Component,
        "Vanilla option kind for barrier payoff evaluation."
    ),
    finstack_quant_core::schema_artifact!(
        OptionType,
        "models",
        "option_type",
        Component,
        "Option payoff direction used by analytical and numerical model engines."
    ),
    finstack_quant_core::schema_artifact!(
        ParametricEsDecompositionView,
        "models",
        "parametric_es_decomposition_view",
        Output,
        "Serializable Expected Shortfall decomposition view."
    ),
    finstack_quant_core::schema_artifact!(
        PdCycleParams,
        "models",
        "pd_cycle_params",
        Input,
        "Parameters for the Merton-Vasicek single-factor PiT/TtC conversion."
    ),
    finstack_quant_core::schema_artifact!(
        PeriodDecomposition,
        "models",
        "period_decomposition",
        Output,
        "Difference between two `LevelsAtDate` snapshots."
    ),
    finstack_quant_core::schema_artifact!(
        PoolGranularity,
        "models",
        "pool_granularity",
        Component,
        "AssetPool-granularity policy for the structured-credit default engine."
    ),
    finstack_quant_core::schema_artifact!(
        PortfolioLossConfig,
        "models",
        "portfolio_loss_config",
        Input,
        "Portfolio credit-loss simulation settings."
    ),
    finstack_quant_core::schema_artifact!(
        PortfolioLossResult,
        "models",
        "portfolio_loss_result",
        Output,
        "Portfolio credit-loss distribution and tail statistics."
    ),
    finstack_quant_core::schema_artifact!(
        PositionRiskDecomposition,
        "models",
        "position_risk_decomposition",
        Output,
        "Complete position-level risk decomposition of a portfolio."
    ),
    finstack_quant_core::schema_artifact!(
        RatingFactorTable,
        "models",
        "rating_factor_table",
        Component,
        "Rating factor table for a specific rating-agency methodology."
    ),
    finstack_quant_core::schema_artifact!(
        RatingPath,
        "models",
        "rating_path",
        Component,
        "A simulated rating trajectory: sequence of (time, state_index) pairs."
    ),
    finstack_quant_core::schema_artifact!(
        RecoveryClaim,
        "models",
        "recovery_claim",
        Component,
        "A claim participating in a recovery waterfall."
    ),
    finstack_quant_core::schema_artifact!(
        RecoverySpec,
        "models",
        "recovery_spec",
        Input,
        "Recovery model specification for configuration and serialization."
    ),
    finstack_quant_core::schema_artifact!(
        RecoveryWaterfallResult,
        "models",
        "recovery_waterfall_result",
        Output,
        "Result of allocating a distributable estate across claims."
    ),
    finstack_quant_core::schema_artifact!(
        RiskBudgetResult,
        "models",
        "risk_budget_result",
        Output,
        "Result of comparing actual risk decomposition against a risk budget."
    ),
    finstack_quant_core::schema_artifact!(
        RiskDecomposition,
        "models",
        "risk_decomposition",
        Output,
        "Portfolio-level decomposition of total risk across common factors and residuals."
    ),
    finstack_quant_core::schema_artifact!(
        RoughBergomiParams,
        "models",
        "rough_bergomi_params",
        Input,
        "rBergomi model parameters."
    ),
    finstack_quant_core::schema_artifact!(
        RoughHestonFourierParams,
        "models",
        "rough_heston_fourier_params",
        Input,
        "Rough Heston model parameters for Fourier-based European option pricing."
    ),
    finstack_quant_core::schema_artifact!(
        RoughHestonParams,
        "models",
        "rough_heston_params",
        Input,
        "Rough Heston model parameters."
    ),
    finstack_quant_core::schema_artifact!(
        SabrMarketData,
        "models",
        "sabr_market_data",
        Component,
        "Market data for SABR calibration."
    ),
    finstack_quant_core::schema_artifact!(
        SabrParameters,
        "models",
        "sabr_parameters",
        Input,
        "SABR model parameters"
    ),
    finstack_quant_core::schema_artifact!(
        SchwartzSmithParams,
        "models",
        "schwartz_smith_params",
        Input,
        "Schwartz-Smith process parameters."
    ),
    finstack_quant_core::schema_artifact!(
        ScoringResult,
        "models",
        "scoring_result",
        Output,
        "Result from any academic scoring model."
    ),
    finstack_quant_core::schema_artifact!(
        SeniorityCalibration,
        "models",
        "seniority_calibration",
        Component,
        "Historical recovery calibration by seniority class."
    ),
    finstack_quant_core::schema_artifact!(
        SeniorityRecovery,
        "models",
        "seniority_recovery",
        Component,
        "Recovery model driven by seniority-class Beta distributions."
    ),
    finstack_quant_core::schema_artifact!(
        SimulatedPaths,
        "models",
        "simulated_paths",
        Component,
        "Results from Monte Carlo path simulation."
    ),
    finstack_quant_core::schema_artifact!(
        StochasticDefaultSpec,
        "models",
        "stochastic_default_spec",
        Input,
        "Stochastic default model specification."
    ),
    finstack_quant_core::schema_artifact!(
        StochasticPrepaySpec,
        "models",
        "stochastic_prepay_spec",
        Input,
        "Stochastic prepayment model specification."
    ),
    finstack_quant_core::schema_artifact!(
        StressAttribution,
        "models",
        "stress_attribution",
        Output,
        "Per-position attribution of portfolio losses in tail scenarios."
    ),
    finstack_quant_core::schema_artifact!(
        SviParams,
        "models",
        "svi_params",
        Input,
        "SVI (Stochastic Volatility Inspired) raw parameterization."
    ),
    finstack_quant_core::schema_artifact!(
        ToggleExerciseModel,
        "models",
        "toggle_exercise_model",
        Input,
        "Toggle exercise model for PIK/cash decision."
    ),
    finstack_quant_core::schema_artifact!(
        TradeParams,
        "models",
        "trade_params",
        Input,
        "Input parameters for a market impact calculation."
    ),
    finstack_quant_core::schema_artifact!(
        TrancheLossStatistics,
        "models",
        "tranche_loss_statistics",
        Output,
        "Loss statistics for one attachment/detachment tranche over a simulated pool loss distribution."
    ),
    finstack_quant_core::schema_artifact!(
        TransitionMatrix,
        "models",
        "transition_matrix",
        Component,
        "Row-stochastic N×N transition matrix representing migration probabilities over a fixed time horizon."
    ),
    finstack_quant_core::schema_artifact!(
        VarianceGammaCf,
        "models",
        "variance_gamma_cf",
        Component,
        "Variance Gamma model characteristic function."
    ),
    finstack_quant_core::schema_artifact!(
        VolatilityConvention,
        "models",
        "volatility_convention",
        Component,
        "Volatility quoting convention."
    ),
    finstack_quant_core::schema_artifact!(
        WorkoutLgd,
        "models",
        "workout_lgd",
        Component,
        "Workout-based LGD model using a collateral-first recovery waterfall."
    ),
    finstack_quant_core::schema_artifact!(
        WorkoutLgdResult,
        "models",
        "workout_lgd_result",
        Output,
        "Outcome of evaluating a `WorkoutLgd` model at one exposure at default."
    ),
    finstack_quant_core::schema_artifact!(
        YieldForecast,
        "models",
        "yield_forecast",
        Output,
        "h-step ahead yield curve forecast with confidence bands."
    ),
    finstack_quant_core::schema_artifact!(
        YieldPanel,
        "models",
        "yield_panel",
        Component,
        "A panel of yield observations: rows = dates, columns = tenors."
    ),
    finstack_quant_core::schema_artifact!(
        YieldPca,
        "models",
        "yield_pca",
        Component,
        "PCA decomposition of yield curve changes."
    ),
    finstack_quant_core::schema_artifact!(
        YieldPcaView,
        "models",
        "yield_pca_view",
        Output,
        "Serializable view of the leading components of a `YieldPca` fit."
    ),
    finstack_quant_core::schema_artifact!(
        ZmijewskiInput,
        "models",
        "zmijewski_input",
        Input,
        "Input for the Zmijewski (1984) probit bankruptcy prediction model."
    ),
];
