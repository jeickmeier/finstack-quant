// Generated consumer import harness. Worker entry executes only through FinstackProvider.
export const importHarnesses = {
  "finstack-base": () =>
    Promise.all([]).then((modules) => ({
      name: "finstack-base",
      modules: modules.length,
    })),
  "use-finstack": () =>
    Promise.all([
      import("@/hooks/use-finstack/use-finstack"),
      import("@/hooks/use-finstack/client"),
      import("@/hooks/use-finstack/version"),
    ]).then((modules) => ({ name: "use-finstack", modules: modules.length })),
  "use-price-instrument": () =>
    Promise.all([
      import("@/hooks/use-price-instrument/use-price-instrument"),
    ]).then((modules) => ({
      name: "use-price-instrument",
      modules: modules.length,
    })),
  "use-validate-instrument": () =>
    Promise.all([
      import("@/hooks/use-validate-instrument/use-validate-instrument"),
    ]).then((modules) => ({
      name: "use-validate-instrument",
      modules: modules.length,
    })),
  "use-linked-selection": () =>
    Promise.all([
      import("@/hooks/use-linked-selection/use-linked-selection"),
    ]).then((modules) => ({
      name: "use-linked-selection",
      modules: modules.length,
    })),
  "use-instrument-validator": () =>
    Promise.all([
      import("@/hooks/use-instrument-validator/use-instrument-validator"),
    ]).then((modules) => ({
      name: "use-instrument-validator",
      modules: modules.length,
    })),
  "use-cashflows": () =>
    Promise.all([import("@/hooks/use-cashflows/use-cashflows")]).then(
      (modules) => ({ name: "use-cashflows", modules: modules.length }),
    ),
  "use-market-validator": () =>
    Promise.all([
      import("@/hooks/use-market-validator/use-market-validator"),
    ]).then((modules) => ({
      name: "use-market-validator",
      modules: modules.length,
    })),
  "use-fx-delta-samples": () =>
    Promise.all([
      import("@/hooks/use-fx-delta-samples/use-fx-delta-samples"),
    ]).then((modules) => ({
      name: "use-fx-delta-samples",
      modules: modules.length,
    })),
  "use-cube-samples": () =>
    Promise.all([import("@/hooks/use-cube-samples/use-cube-samples")]).then(
      (modules) => ({ name: "use-cube-samples", modules: modules.length }),
    ),
  "use-calibrate": () =>
    Promise.all([import("@/hooks/use-calibrate/use-calibrate")]).then(
      (modules) => ({ name: "use-calibrate", modules: modules.length }),
    ),
  "use-scenario-table": () =>
    Promise.all([import("@/hooks/use-scenario-table/use-scenario-table")]).then(
      (modules) => ({ name: "use-scenario-table", modules: modules.length }),
    ),
  "finstack-worker": () =>
    Promise.all([
      import("@/workers/finstack-service"),
      import("@/workers/finstack-contract"),
    ]).then((modules) => ({
      name: "finstack-worker",
      modules: modules.length,
    })),
  "finstack-fixtures": () =>
    Promise.all([import("@/lib/finstack/fixtures/results/bond.json")]).then(
      (modules) => ({ name: "finstack-fixtures", modules: modules.length }),
    ),
  "primitive-contracts": () =>
    Promise.all([
      import("@/lib/finstack/generated/primitive-contracts.json"),
    ]).then((modules) => ({
      name: "primitive-contracts",
      modules: modules.length,
    })),
  "finstack-format": () =>
    Promise.all([
      import("@/lib/finstack/format/format"),
      import("@/lib/finstack/format/columns"),
      import("@/lib/finstack/format/transport"),
    ]).then((modules) => ({
      name: "finstack-format",
      modules: modules.length,
    })),
  "finstack-codec": () =>
    Promise.all([
      import("@/lib/finstack/codec.mjs"),
      import("@/lib/finstack/schema.mjs"),
    ]).then((modules) => ({ name: "finstack-codec", modules: modules.length })),
  "contract-calibration": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/calibration.json"),
      import("@/lib/finstack/generated/types/calibration"),
      import("@/lib/finstack/generated/meta/calibration"),
    ]).then((modules) => ({
      name: "contract-calibration",
      modules: modules.length,
    })),
  "contract-commodity-asian-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_asian_option.json"),
      import("@/lib/finstack/generated/types/commodity_asian_option"),
      import("@/lib/finstack/generated/meta/commodity_asian_option"),
      import("@/lib/finstack/generated/instrument/commodity_asian_option"),
      import("@/lib/finstack/generated/examples/commodity_asian_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-asian-option",
      modules: modules.length,
    })),
  "contract-commodity-forward": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_forward.json"),
      import("@/lib/finstack/generated/types/commodity_forward"),
      import("@/lib/finstack/generated/meta/commodity_forward"),
      import("@/lib/finstack/generated/instrument/commodity_forward"),
      import("@/lib/finstack/generated/examples/commodity_forward.json"),
    ]).then((modules) => ({
      name: "contract-commodity-forward",
      modules: modules.length,
    })),
  "contract-commodity-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_future_option.json"),
      import("@/lib/finstack/generated/types/commodity_future_option"),
      import("@/lib/finstack/generated/meta/commodity_future_option"),
      import("@/lib/finstack/generated/instrument/commodity_future_option"),
      import("@/lib/finstack/generated/examples/commodity_future_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-future-option",
      modules: modules.length,
    })),
  "contract-commodity-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_future.json"),
      import("@/lib/finstack/generated/types/commodity_future"),
      import("@/lib/finstack/generated/meta/commodity_future"),
      import("@/lib/finstack/generated/instrument/commodity_future"),
      import("@/lib/finstack/generated/examples/commodity_future.json"),
    ]).then((modules) => ({
      name: "contract-commodity-future",
      modules: modules.length,
    })),
  "contract-commodity-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_option.json"),
      import("@/lib/finstack/generated/types/commodity_option"),
      import("@/lib/finstack/generated/meta/commodity_option"),
      import("@/lib/finstack/generated/instrument/commodity_option"),
      import("@/lib/finstack/generated/examples/commodity_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-option",
      modules: modules.length,
    })),
  "contract-commodity-spread-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_spread_option.json"),
      import("@/lib/finstack/generated/types/commodity_spread_option"),
      import("@/lib/finstack/generated/meta/commodity_spread_option"),
      import("@/lib/finstack/generated/instrument/commodity_spread_option"),
      import("@/lib/finstack/generated/examples/commodity_spread_option.json"),
    ]).then((modules) => ({
      name: "contract-commodity-spread-option",
      modules: modules.length,
    })),
  "contract-commodity-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_swap.json"),
      import("@/lib/finstack/generated/types/commodity_swap"),
      import("@/lib/finstack/generated/meta/commodity_swap"),
      import("@/lib/finstack/generated/instrument/commodity_swap"),
      import("@/lib/finstack/generated/examples/commodity_swap.json"),
    ]).then((modules) => ({
      name: "contract-commodity-swap",
      modules: modules.length,
    })),
  "contract-commodity-swaption": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/commodity_swaption.json"),
      import("@/lib/finstack/generated/types/commodity_swaption"),
      import("@/lib/finstack/generated/meta/commodity_swaption"),
      import("@/lib/finstack/generated/instrument/commodity_swaption"),
      import("@/lib/finstack/generated/examples/commodity_swaption.json"),
    ]).then((modules) => ({
      name: "contract-commodity-swaption",
      modules: modules.length,
    })),
  "contract-composite": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/composite.json"),
      import("@/lib/finstack/generated/types/composite"),
      import("@/lib/finstack/generated/meta/composite"),
      import("@/lib/finstack/generated/instrument/composite"),
      import("@/lib/finstack/generated/examples/composite.json"),
    ]).then((modules) => ({
      name: "contract-composite",
      modules: modules.length,
    })),
  "contract-cds-index": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cds_index.json"),
      import("@/lib/finstack/generated/types/cds_index"),
      import("@/lib/finstack/generated/meta/cds_index"),
      import("@/lib/finstack/generated/instrument/cds_index"),
      import("@/lib/finstack/generated/examples/cds_index.json"),
    ]).then((modules) => ({
      name: "contract-cds-index",
      modules: modules.length,
    })),
  "contract-cds-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cds_option.json"),
      import("@/lib/finstack/generated/types/cds_option"),
      import("@/lib/finstack/generated/meta/cds_option"),
      import("@/lib/finstack/generated/instrument/cds_option"),
      import("@/lib/finstack/generated/examples/cds_option.json"),
    ]).then((modules) => ({
      name: "contract-cds-option",
      modules: modules.length,
    })),
  "contract-cds-tranche": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cds_tranche.json"),
      import("@/lib/finstack/generated/types/cds_tranche"),
      import("@/lib/finstack/generated/meta/cds_tranche"),
      import("@/lib/finstack/generated/instrument/cds_tranche"),
      import("@/lib/finstack/generated/examples/cds_tranche.json"),
    ]).then((modules) => ({
      name: "contract-cds-tranche",
      modules: modules.length,
    })),
  "contract-credit-default-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/credit_default_swap.json"),
      import("@/lib/finstack/generated/types/credit_default_swap"),
      import("@/lib/finstack/generated/meta/credit_default_swap"),
      import("@/lib/finstack/generated/instrument/credit_default_swap"),
      import("@/lib/finstack/generated/examples/credit_default_swap.json"),
    ]).then((modules) => ({
      name: "contract-credit-default-swap",
      modules: modules.length,
    })),
  "contract-autocallable": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/autocallable.json"),
      import("@/lib/finstack/generated/types/autocallable"),
      import("@/lib/finstack/generated/meta/autocallable"),
      import("@/lib/finstack/generated/instrument/autocallable"),
      import("@/lib/finstack/generated/examples/autocallable.json"),
    ]).then((modules) => ({
      name: "contract-autocallable",
      modules: modules.length,
    })),
  "contract-cliquet-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cliquet_option.json"),
      import("@/lib/finstack/generated/types/cliquet_option"),
      import("@/lib/finstack/generated/meta/cliquet_option"),
      import("@/lib/finstack/generated/instrument/cliquet_option"),
      import("@/lib/finstack/generated/examples/cliquet_option.json"),
    ]).then((modules) => ({
      name: "contract-cliquet-option",
      modules: modules.length,
    })),
  "contract-discounted-cash-flow": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/discounted_cash_flow.json"),
      import("@/lib/finstack/generated/types/discounted_cash_flow"),
      import("@/lib/finstack/generated/meta/discounted_cash_flow"),
      import("@/lib/finstack/generated/instrument/discounted_cash_flow"),
      import("@/lib/finstack/generated/examples/discounted_cash_flow.json"),
    ]).then((modules) => ({
      name: "contract-discounted-cash-flow",
      modules: modules.length,
    })),
  "contract-equity-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_future_option.json"),
      import("@/lib/finstack/generated/types/equity_future_option"),
      import("@/lib/finstack/generated/meta/equity_future_option"),
      import("@/lib/finstack/generated/instrument/equity_future_option"),
      import("@/lib/finstack/generated/examples/equity_future_option.json"),
    ]).then((modules) => ({
      name: "contract-equity-future-option",
      modules: modules.length,
    })),
  "contract-equity-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_future.json"),
      import("@/lib/finstack/generated/types/equity_future"),
      import("@/lib/finstack/generated/meta/equity_future"),
      import("@/lib/finstack/generated/instrument/equity_future"),
      import("@/lib/finstack/generated/examples/equity_future.json"),
    ]).then((modules) => ({
      name: "contract-equity-future",
      modules: modules.length,
    })),
  "contract-equity-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_option.json"),
      import("@/lib/finstack/generated/types/equity_option"),
      import("@/lib/finstack/generated/meta/equity_option"),
      import("@/lib/finstack/generated/instrument/equity_option"),
      import("@/lib/finstack/generated/examples/equity_option.json"),
    ]).then((modules) => ({
      name: "contract-equity-option",
      modules: modules.length,
    })),
  "contract-equity-total-return-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity_total_return_future.json"),
      import("@/lib/finstack/generated/types/equity_total_return_future"),
      import("@/lib/finstack/generated/meta/equity_total_return_future"),
      import("@/lib/finstack/generated/instrument/equity_total_return_future"),
      import("@/lib/finstack/generated/examples/equity_total_return_future.json"),
    ]).then((modules) => ({
      name: "contract-equity-total-return-future",
      modules: modules.length,
    })),
  "contract-equity": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/equity.json"),
      import("@/lib/finstack/generated/types/equity"),
      import("@/lib/finstack/generated/meta/equity"),
      import("@/lib/finstack/generated/instrument/equity"),
      import("@/lib/finstack/generated/examples/equity.json"),
    ]).then((modules) => ({
      name: "contract-equity",
      modules: modules.length,
    })),
  "contract-levered-real-estate-equity": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/levered_real_estate_equity.json"),
      import("@/lib/finstack/generated/types/levered_real_estate_equity"),
      import("@/lib/finstack/generated/meta/levered_real_estate_equity"),
      import("@/lib/finstack/generated/instrument/levered_real_estate_equity"),
      import("@/lib/finstack/generated/examples/levered_real_estate_equity.json"),
    ]).then((modules) => ({
      name: "contract-levered-real-estate-equity",
      modules: modules.length,
    })),
  "contract-private-markets-fund": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/private_markets_fund.json"),
      import("@/lib/finstack/generated/types/private_markets_fund"),
      import("@/lib/finstack/generated/meta/private_markets_fund"),
      import("@/lib/finstack/generated/instrument/private_markets_fund"),
      import("@/lib/finstack/generated/examples/private_markets_fund.json"),
    ]).then((modules) => ({
      name: "contract-private-markets-fund",
      modules: modules.length,
    })),
  "contract-real-estate-asset": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/real_estate_asset.json"),
      import("@/lib/finstack/generated/types/real_estate_asset"),
      import("@/lib/finstack/generated/meta/real_estate_asset"),
      import("@/lib/finstack/generated/instrument/real_estate_asset"),
      import("@/lib/finstack/generated/examples/real_estate_asset.json"),
    ]).then((modules) => ({
      name: "contract-real-estate-asset",
      modules: modules.length,
    })),
  "contract-trs-equity": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/trs_equity.json"),
      import("@/lib/finstack/generated/types/trs_equity"),
      import("@/lib/finstack/generated/meta/trs_equity"),
      import("@/lib/finstack/generated/instrument/trs_equity"),
      import("@/lib/finstack/generated/examples/trs_equity.json"),
    ]).then((modules) => ({
      name: "contract-trs-equity",
      modules: modules.length,
    })),
  "contract-variance-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/variance_swap.json"),
      import("@/lib/finstack/generated/types/variance_swap"),
      import("@/lib/finstack/generated/meta/variance_swap"),
      import("@/lib/finstack/generated/instrument/variance_swap"),
      import("@/lib/finstack/generated/examples/variance_swap.json"),
    ]).then((modules) => ({
      name: "contract-variance-swap",
      modules: modules.length,
    })),
  "contract-volatility-index-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/volatility_index_future_option.json"),
      import("@/lib/finstack/generated/types/volatility_index_future_option"),
      import("@/lib/finstack/generated/meta/volatility_index_future_option"),
      import("@/lib/finstack/generated/instrument/volatility_index_future_option"),
      import("@/lib/finstack/generated/examples/volatility_index_future_option.json"),
    ]).then((modules) => ({
      name: "contract-volatility-index-future-option",
      modules: modules.length,
    })),
  "contract-volatility-index-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/volatility_index_future.json"),
      import("@/lib/finstack/generated/types/volatility_index_future"),
      import("@/lib/finstack/generated/meta/volatility_index_future"),
      import("@/lib/finstack/generated/instrument/volatility_index_future"),
      import("@/lib/finstack/generated/examples/volatility_index_future.json"),
    ]).then((modules) => ({
      name: "contract-volatility-index-future",
      modules: modules.length,
    })),
  "contract-asian-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/asian_option.json"),
      import("@/lib/finstack/generated/types/asian_option"),
      import("@/lib/finstack/generated/meta/asian_option"),
      import("@/lib/finstack/generated/instrument/asian_option"),
      import("@/lib/finstack/generated/examples/asian_option.json"),
    ]).then((modules) => ({
      name: "contract-asian-option",
      modules: modules.length,
    })),
  "contract-barrier-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/barrier_option.json"),
      import("@/lib/finstack/generated/types/barrier_option"),
      import("@/lib/finstack/generated/meta/barrier_option"),
      import("@/lib/finstack/generated/instrument/barrier_option"),
      import("@/lib/finstack/generated/examples/barrier_option.json"),
    ]).then((modules) => ({
      name: "contract-barrier-option",
      modules: modules.length,
    })),
  "contract-basket": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/basket.json"),
      import("@/lib/finstack/generated/types/basket"),
      import("@/lib/finstack/generated/meta/basket"),
      import("@/lib/finstack/generated/instrument/basket"),
      import("@/lib/finstack/generated/examples/basket.json"),
    ]).then((modules) => ({
      name: "contract-basket",
      modules: modules.length,
    })),
  "contract-callable-range-accrual": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/callable_range_accrual.json"),
      import("@/lib/finstack/generated/types/callable_range_accrual"),
      import("@/lib/finstack/generated/meta/callable_range_accrual"),
      import("@/lib/finstack/generated/instrument/callable_range_accrual"),
      import("@/lib/finstack/generated/examples/callable_range_accrual.json"),
    ]).then((modules) => ({
      name: "contract-callable-range-accrual",
      modules: modules.length,
    })),
  "contract-lookback-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/lookback_option.json"),
      import("@/lib/finstack/generated/types/lookback_option"),
      import("@/lib/finstack/generated/meta/lookback_option"),
      import("@/lib/finstack/generated/instrument/lookback_option"),
      import("@/lib/finstack/generated/examples/lookback_option.json"),
    ]).then((modules) => ({
      name: "contract-lookback-option",
      modules: modules.length,
    })),
  "contract-range-accrual": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/range_accrual.json"),
      import("@/lib/finstack/generated/types/range_accrual"),
      import("@/lib/finstack/generated/meta/range_accrual"),
      import("@/lib/finstack/generated/instrument/range_accrual"),
      import("@/lib/finstack/generated/examples/range_accrual.json"),
    ]).then((modules) => ({
      name: "contract-range-accrual",
      modules: modules.length,
    })),
  "contract-snowball": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/snowball.json"),
      import("@/lib/finstack/generated/types/snowball"),
      import("@/lib/finstack/generated/meta/snowball"),
      import("@/lib/finstack/generated/instrument/snowball"),
      import("@/lib/finstack/generated/examples/snowball.json"),
    ]).then((modules) => ({
      name: "contract-snowball",
      modules: modules.length,
    })),
  "contract-tarn": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/tarn.json"),
      import("@/lib/finstack/generated/types/tarn"),
      import("@/lib/finstack/generated/meta/tarn"),
      import("@/lib/finstack/generated/instrument/tarn"),
      import("@/lib/finstack/generated/examples/tarn.json"),
    ]).then((modules) => ({ name: "contract-tarn", modules: modules.length })),
  "contract-agency-cmo": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/agency_cmo.json"),
      import("@/lib/finstack/generated/types/agency_cmo"),
      import("@/lib/finstack/generated/meta/agency_cmo"),
      import("@/lib/finstack/generated/instrument/agency_cmo"),
      import("@/lib/finstack/generated/examples/agency_cmo.json"),
    ]).then((modules) => ({
      name: "contract-agency-cmo",
      modules: modules.length,
    })),
  "contract-agency-mbs-passthrough": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/agency_mbs_passthrough.json"),
      import("@/lib/finstack/generated/types/agency_mbs_passthrough"),
      import("@/lib/finstack/generated/meta/agency_mbs_passthrough"),
      import("@/lib/finstack/generated/instrument/agency_mbs_passthrough"),
      import("@/lib/finstack/generated/examples/agency_mbs_passthrough.json"),
    ]).then((modules) => ({
      name: "contract-agency-mbs-passthrough",
      modules: modules.length,
    })),
  "contract-agency-tba": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/agency_tba.json"),
      import("@/lib/finstack/generated/types/agency_tba"),
      import("@/lib/finstack/generated/meta/agency_tba"),
      import("@/lib/finstack/generated/instrument/agency_tba"),
      import("@/lib/finstack/generated/examples/agency_tba.json"),
    ]).then((modules) => ({
      name: "contract-agency-tba",
      modules: modules.length,
    })),
  "contract-asset-backed-facility": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/asset_backed_facility.json"),
      import("@/lib/finstack/generated/types/asset_backed_facility"),
      import("@/lib/finstack/generated/meta/asset_backed_facility"),
      import("@/lib/finstack/generated/instrument/asset_backed_facility"),
      import("@/lib/finstack/generated/examples/asset_backed_facility.json"),
    ]).then((modules) => ({
      name: "contract-asset-backed-facility",
      modules: modules.length,
    })),
  "contract-bond-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bond_future.json"),
      import("@/lib/finstack/generated/types/bond_future"),
      import("@/lib/finstack/generated/meta/bond_future"),
      import("@/lib/finstack/generated/instrument/bond_future"),
      import("@/lib/finstack/generated/examples/bond_future.json"),
    ]).then((modules) => ({
      name: "contract-bond-future",
      modules: modules.length,
    })),
  "contract-bond": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bond.json"),
      import("@/lib/finstack/generated/types/bond"),
      import("@/lib/finstack/generated/meta/bond"),
      import("@/lib/finstack/generated/instrument/bond"),
      import("@/lib/finstack/generated/examples/bond.json"),
    ]).then((modules) => ({ name: "contract-bond", modules: modules.length })),
  "contract-convertible-bond": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/convertible_bond.json"),
      import("@/lib/finstack/generated/types/convertible_bond"),
      import("@/lib/finstack/generated/meta/convertible_bond"),
      import("@/lib/finstack/generated/instrument/convertible_bond"),
      import("@/lib/finstack/generated/examples/convertible_bond.json"),
    ]).then((modules) => ({
      name: "contract-convertible-bond",
      modules: modules.length,
    })),
  "contract-dollar-roll": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/dollar_roll.json"),
      import("@/lib/finstack/generated/types/dollar_roll"),
      import("@/lib/finstack/generated/meta/dollar_roll"),
      import("@/lib/finstack/generated/instrument/dollar_roll"),
      import("@/lib/finstack/generated/examples/dollar_roll.json"),
    ]).then((modules) => ({
      name: "contract-dollar-roll",
      modules: modules.length,
    })),
  "contract-inflation-linked-bond": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/inflation_linked_bond.json"),
      import("@/lib/finstack/generated/types/inflation_linked_bond"),
      import("@/lib/finstack/generated/meta/inflation_linked_bond"),
      import("@/lib/finstack/generated/instrument/inflation_linked_bond"),
      import("@/lib/finstack/generated/examples/inflation_linked_bond.json"),
    ]).then((modules) => ({
      name: "contract-inflation-linked-bond",
      modules: modules.length,
    })),
  "contract-revolving-credit": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/revolving_credit.json"),
      import("@/lib/finstack/generated/types/revolving_credit"),
      import("@/lib/finstack/generated/meta/revolving_credit"),
      import("@/lib/finstack/generated/instrument/revolving_credit"),
      import("@/lib/finstack/generated/examples/revolving_credit.json"),
    ]).then((modules) => ({
      name: "contract-revolving-credit",
      modules: modules.length,
    })),
  "contract-structured-credit": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/structured_credit.json"),
      import("@/lib/finstack/generated/types/structured_credit"),
      import("@/lib/finstack/generated/meta/structured_credit"),
      import("@/lib/finstack/generated/instrument/structured_credit"),
      import("@/lib/finstack/generated/examples/structured_credit.json"),
    ]).then((modules) => ({
      name: "contract-structured-credit",
      modules: modules.length,
    })),
  "contract-term-loan": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/term_loan.json"),
      import("@/lib/finstack/generated/types/term_loan"),
      import("@/lib/finstack/generated/meta/term_loan"),
      import("@/lib/finstack/generated/instrument/term_loan"),
      import("@/lib/finstack/generated/examples/term_loan.json"),
    ]).then((modules) => ({
      name: "contract-term-loan",
      modules: modules.length,
    })),
  "contract-trs-fixed-income-index": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/trs_fixed_income_index.json"),
      import("@/lib/finstack/generated/types/trs_fixed_income_index"),
      import("@/lib/finstack/generated/meta/trs_fixed_income_index"),
      import("@/lib/finstack/generated/instrument/trs_fixed_income_index"),
      import("@/lib/finstack/generated/examples/trs_fixed_income_index.json"),
    ]).then((modules) => ({
      name: "contract-trs-fixed-income-index",
      modules: modules.length,
    })),
  "contract-fx-barrier-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_barrier_option.json"),
      import("@/lib/finstack/generated/types/fx_barrier_option"),
      import("@/lib/finstack/generated/meta/fx_barrier_option"),
      import("@/lib/finstack/generated/instrument/fx_barrier_option"),
      import("@/lib/finstack/generated/examples/fx_barrier_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-barrier-option",
      modules: modules.length,
    })),
  "contract-fx-digital-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_digital_option.json"),
      import("@/lib/finstack/generated/types/fx_digital_option"),
      import("@/lib/finstack/generated/meta/fx_digital_option"),
      import("@/lib/finstack/generated/instrument/fx_digital_option"),
      import("@/lib/finstack/generated/examples/fx_digital_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-digital-option",
      modules: modules.length,
    })),
  "contract-fx-forward": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_forward.json"),
      import("@/lib/finstack/generated/types/fx_forward"),
      import("@/lib/finstack/generated/meta/fx_forward"),
      import("@/lib/finstack/generated/instrument/fx_forward"),
      import("@/lib/finstack/generated/examples/fx_forward.json"),
    ]).then((modules) => ({
      name: "contract-fx-forward",
      modules: modules.length,
    })),
  "contract-fx-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_future_option.json"),
      import("@/lib/finstack/generated/types/fx_future_option"),
      import("@/lib/finstack/generated/meta/fx_future_option"),
      import("@/lib/finstack/generated/instrument/fx_future_option"),
      import("@/lib/finstack/generated/examples/fx_future_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-future-option",
      modules: modules.length,
    })),
  "contract-fx-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_future.json"),
      import("@/lib/finstack/generated/types/fx_future"),
      import("@/lib/finstack/generated/meta/fx_future"),
      import("@/lib/finstack/generated/instrument/fx_future"),
      import("@/lib/finstack/generated/examples/fx_future.json"),
    ]).then((modules) => ({
      name: "contract-fx-future",
      modules: modules.length,
    })),
  "contract-fx-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_option.json"),
      import("@/lib/finstack/generated/types/fx_option"),
      import("@/lib/finstack/generated/meta/fx_option"),
      import("@/lib/finstack/generated/instrument/fx_option"),
      import("@/lib/finstack/generated/examples/fx_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-option",
      modules: modules.length,
    })),
  "contract-fx-spot": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_spot.json"),
      import("@/lib/finstack/generated/types/fx_spot"),
      import("@/lib/finstack/generated/meta/fx_spot"),
      import("@/lib/finstack/generated/instrument/fx_spot"),
      import("@/lib/finstack/generated/examples/fx_spot.json"),
    ]).then((modules) => ({
      name: "contract-fx-spot",
      modules: modules.length,
    })),
  "contract-fx-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_swap.json"),
      import("@/lib/finstack/generated/types/fx_swap"),
      import("@/lib/finstack/generated/meta/fx_swap"),
      import("@/lib/finstack/generated/instrument/fx_swap"),
      import("@/lib/finstack/generated/examples/fx_swap.json"),
    ]).then((modules) => ({
      name: "contract-fx-swap",
      modules: modules.length,
    })),
  "contract-fx-touch-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_touch_option.json"),
      import("@/lib/finstack/generated/types/fx_touch_option"),
      import("@/lib/finstack/generated/meta/fx_touch_option"),
      import("@/lib/finstack/generated/instrument/fx_touch_option"),
      import("@/lib/finstack/generated/examples/fx_touch_option.json"),
    ]).then((modules) => ({
      name: "contract-fx-touch-option",
      modules: modules.length,
    })),
  "contract-fx-variance-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/fx_variance_swap.json"),
      import("@/lib/finstack/generated/types/fx_variance_swap"),
      import("@/lib/finstack/generated/meta/fx_variance_swap"),
      import("@/lib/finstack/generated/instrument/fx_variance_swap"),
      import("@/lib/finstack/generated/examples/fx_variance_swap.json"),
    ]).then((modules) => ({
      name: "contract-fx-variance-swap",
      modules: modules.length,
    })),
  "contract-ndf": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/ndf.json"),
      import("@/lib/finstack/generated/types/ndf"),
      import("@/lib/finstack/generated/meta/ndf"),
      import("@/lib/finstack/generated/instrument/ndf"),
      import("@/lib/finstack/generated/examples/ndf.json"),
    ]).then((modules) => ({ name: "contract-ndf", modules: modules.length })),
  "contract-quanto-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/quanto_option.json"),
      import("@/lib/finstack/generated/types/quanto_option"),
      import("@/lib/finstack/generated/meta/quanto_option"),
      import("@/lib/finstack/generated/instrument/quanto_option"),
      import("@/lib/finstack/generated/examples/quanto_option.json"),
    ]).then((modules) => ({
      name: "contract-quanto-option",
      modules: modules.length,
    })),
  "contract-basis-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/basis_swap.json"),
      import("@/lib/finstack/generated/types/basis_swap"),
      import("@/lib/finstack/generated/meta/basis_swap"),
      import("@/lib/finstack/generated/instrument/basis_swap"),
      import("@/lib/finstack/generated/examples/basis_swap.json"),
    ]).then((modules) => ({
      name: "contract-basis-swap",
      modules: modules.length,
    })),
  "contract-bermudan-swaption": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/bermudan_swaption.json"),
      import("@/lib/finstack/generated/types/bermudan_swaption"),
      import("@/lib/finstack/generated/meta/bermudan_swaption"),
      import("@/lib/finstack/generated/instrument/bermudan_swaption"),
      import("@/lib/finstack/generated/examples/bermudan_swaption.json"),
    ]).then((modules) => ({
      name: "contract-bermudan-swaption",
      modules: modules.length,
    })),
  "contract-cap-floor": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cap_floor.json"),
      import("@/lib/finstack/generated/types/cap_floor"),
      import("@/lib/finstack/generated/meta/cap_floor"),
      import("@/lib/finstack/generated/instrument/cap_floor"),
      import("@/lib/finstack/generated/examples/cap_floor.json"),
    ]).then((modules) => ({
      name: "contract-cap-floor",
      modules: modules.length,
    })),
  "contract-cms-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cms_option.json"),
      import("@/lib/finstack/generated/types/cms_option"),
      import("@/lib/finstack/generated/meta/cms_option"),
      import("@/lib/finstack/generated/instrument/cms_option"),
      import("@/lib/finstack/generated/examples/cms_option.json"),
    ]).then((modules) => ({
      name: "contract-cms-option",
      modules: modules.length,
    })),
  "contract-cms-spread-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cms_spread_option.json"),
      import("@/lib/finstack/generated/types/cms_spread_option"),
      import("@/lib/finstack/generated/meta/cms_spread_option"),
      import("@/lib/finstack/generated/instrument/cms_spread_option"),
      import("@/lib/finstack/generated/examples/cms_spread_option.json"),
    ]).then((modules) => ({
      name: "contract-cms-spread-option",
      modules: modules.length,
    })),
  "contract-cms-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/cms_swap.json"),
      import("@/lib/finstack/generated/types/cms_swap"),
      import("@/lib/finstack/generated/meta/cms_swap"),
      import("@/lib/finstack/generated/instrument/cms_swap"),
      import("@/lib/finstack/generated/examples/cms_swap.json"),
    ]).then((modules) => ({
      name: "contract-cms-swap",
      modules: modules.length,
    })),
  "contract-deposit": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/deposit.json"),
      import("@/lib/finstack/generated/types/deposit"),
      import("@/lib/finstack/generated/meta/deposit"),
      import("@/lib/finstack/generated/instrument/deposit"),
      import("@/lib/finstack/generated/examples/deposit.json"),
    ]).then((modules) => ({
      name: "contract-deposit",
      modules: modules.length,
    })),
  "contract-forward-rate-agreement": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/forward_rate_agreement.json"),
      import("@/lib/finstack/generated/types/forward_rate_agreement"),
      import("@/lib/finstack/generated/meta/forward_rate_agreement"),
      import("@/lib/finstack/generated/instrument/forward_rate_agreement"),
      import("@/lib/finstack/generated/examples/forward_rate_agreement.json"),
    ]).then((modules) => ({
      name: "contract-forward-rate-agreement",
      modules: modules.length,
    })),
  "contract-inflation-cap-floor": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/inflation_cap_floor.json"),
      import("@/lib/finstack/generated/types/inflation_cap_floor"),
      import("@/lib/finstack/generated/meta/inflation_cap_floor"),
      import("@/lib/finstack/generated/instrument/inflation_cap_floor"),
      import("@/lib/finstack/generated/examples/inflation_cap_floor.json"),
    ]).then((modules) => ({
      name: "contract-inflation-cap-floor",
      modules: modules.length,
    })),
  "contract-inflation-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/inflation_swap.json"),
      import("@/lib/finstack/generated/types/inflation_swap"),
      import("@/lib/finstack/generated/meta/inflation_swap"),
      import("@/lib/finstack/generated/instrument/inflation_swap"),
      import("@/lib/finstack/generated/examples/inflation_swap.json"),
    ]).then((modules) => ({
      name: "contract-inflation-swap",
      modules: modules.length,
    })),
  "contract-interest-rate-future-option": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/interest_rate_future_option.json"),
      import("@/lib/finstack/generated/types/interest_rate_future_option"),
      import("@/lib/finstack/generated/meta/interest_rate_future_option"),
      import("@/lib/finstack/generated/instrument/interest_rate_future_option"),
      import("@/lib/finstack/generated/examples/interest_rate_future_option.json"),
    ]).then((modules) => ({
      name: "contract-interest-rate-future-option",
      modules: modules.length,
    })),
  "contract-interest-rate-future": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/interest_rate_future.json"),
      import("@/lib/finstack/generated/types/interest_rate_future"),
      import("@/lib/finstack/generated/meta/interest_rate_future"),
      import("@/lib/finstack/generated/instrument/interest_rate_future"),
      import("@/lib/finstack/generated/examples/interest_rate_future.json"),
    ]).then((modules) => ({
      name: "contract-interest-rate-future",
      modules: modules.length,
    })),
  "contract-interest-rate-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/interest_rate_swap.json"),
      import("@/lib/finstack/generated/types/interest_rate_swap"),
      import("@/lib/finstack/generated/meta/interest_rate_swap"),
      import("@/lib/finstack/generated/instrument/interest_rate_swap"),
      import("@/lib/finstack/generated/examples/interest_rate_swap.json"),
    ]).then((modules) => ({
      name: "contract-interest-rate-swap",
      modules: modules.length,
    })),
  "contract-repo": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/repo.json"),
      import("@/lib/finstack/generated/types/repo"),
      import("@/lib/finstack/generated/meta/repo"),
      import("@/lib/finstack/generated/instrument/repo"),
      import("@/lib/finstack/generated/examples/repo.json"),
    ]).then((modules) => ({ name: "contract-repo", modules: modules.length })),
  "contract-swaption": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/swaption.json"),
      import("@/lib/finstack/generated/types/swaption"),
      import("@/lib/finstack/generated/meta/swaption"),
      import("@/lib/finstack/generated/instrument/swaption"),
      import("@/lib/finstack/generated/examples/swaption.json"),
    ]).then((modules) => ({
      name: "contract-swaption",
      modules: modules.length,
    })),
  "contract-xccy-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/xccy_swap.json"),
      import("@/lib/finstack/generated/types/xccy_swap"),
      import("@/lib/finstack/generated/meta/xccy_swap"),
      import("@/lib/finstack/generated/instrument/xccy_swap"),
      import("@/lib/finstack/generated/examples/xccy_swap.json"),
    ]).then((modules) => ({
      name: "contract-xccy-swap",
      modules: modules.length,
    })),
  "contract-yoy-inflation-swap": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/yoy_inflation_swap.json"),
      import("@/lib/finstack/generated/types/yoy_inflation_swap"),
      import("@/lib/finstack/generated/meta/yoy_inflation_swap"),
      import("@/lib/finstack/generated/instrument/yoy_inflation_swap"),
      import("@/lib/finstack/generated/examples/yoy_inflation_swap.json"),
    ]).then((modules) => ({
      name: "contract-yoy-inflation-swap",
      modules: modules.length,
    })),
  "contract-market-context-state": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/market_context_state.json"),
      import("@/lib/finstack/generated/types/market_context_state"),
      import("@/lib/finstack/generated/meta/market_context_state"),
    ]).then((modules) => ({
      name: "contract-market-context-state",
      modules: modules.length,
    })),
  "contract-valuation-result": () =>
    Promise.all([
      import("@/lib/finstack/generated/schemas/valuation_result.json"),
      import("@/lib/finstack/generated/types/valuation_result"),
      import("@/lib/finstack/generated/meta/valuation_result"),
    ]).then((modules) => ({
      name: "contract-valuation-result",
      modules: modules.length,
    })),
  "instrument-catalogue": () =>
    Promise.all([
      import("@/lib/finstack/generated/instruments"),
      import("@/lib/finstack/generated/catalogue.json"),
    ]).then((modules) => ({
      name: "instrument-catalogue",
      modules: modules.length,
    })),
  "contract-manifest": () =>
    Promise.all([
      import("@/lib/finstack/generated/roots.json"),
      import("@/lib/finstack/generated/fixtures.json"),
      import("@/lib/finstack/contract-provenance.json"),
    ]).then((modules) => ({
      name: "contract-manifest",
      modules: modules.length,
    })),
  "finstack-views": () =>
    Promise.all([
      import("@/lib/finstack/views"),
      import("@/lib/finstack/generated/curve-views.json"),
    ]).then((modules) => ({ name: "finstack-views", modules: modules.length })),
  "finstack-host": () =>
    Promise.all([import("@/lib/finstack/host")]).then((modules) => ({
      name: "finstack-host",
      modules: modules.length,
    })),
};
