import { readFile } from "node:fs/promises";

export async function pricingCases() {
  const root = new URL(
    "../../../finstack-quant/valuations/tests/",
    import.meta.url,
  );
  // Resolve from the repository, not from the temporary Next route.
  const read = async (path) =>
    JSON.parse(await readFile(new URL(path, root), "utf8"));
  const bond = await read("instruments/json_examples/bond.json");
  Object.assign(bond.instrument.spec, {
    discount_curve_id: "USD-OIS",
    credit_curve_id: "HZ",
    issue_date: "2025-01-01",
    maturity: "2026-01-01",
    instrument_pricing_overrides: {
      model_config: { hazard_volatility: 0.01, mc_paths: 8, tree_steps: 4 },
    },
  });
  const market = (await read("fixtures/production_cds_option.json")).market;
  // Synthetic 2025-01-01 term structures for the component pricing examples.
  // Discount factors imply a roughly 4% USD OIS curve; hazards are annual decimals.
  const discount = market.curves.find((curve) => curve.id === "USD-OIS");
  const hazard = market.curves.find((curve) => curve.id === "HZ");
  if (!discount || !hazard) throw new Error("Missing example market curves");
  discount.knot_points = [
    [0, 1],
    [0.25, 0.989307575],
    [0.5, 0.97897419],
    [1, 0.959349335],
    [2, 0.923116346],
    [3, 0.889051602],
    [5, 0.824894318],
    [7, 0.762159064],
    [10, 0.675704114],
    [20, 0.449328964],
    [30, 0.296710014],
    [40, 0.193980042],
  ];
  hazard.knot_points = [
    [0, 0.012],
    [1, 0.014],
    [3, 0.016],
    [5, 0.018],
    [10, 0.021],
    [20, 0.025],
    [40, 0.028],
  ];
  const request = {
    instrumentJson: JSON.stringify(bond),
    marketJson: JSON.stringify(market),
    asOf: "2025-01-01",
    model: "discounting",
    metrics: [],
    pricingOptions: undefined,
    marketHistory: undefined,
  };
  const bad = structuredClone(bond);
  bad.instrument.type = "__invalid__";
  return {
    bond: request,
    stochastic: { ...request, model: "rates_credit" },
    badInstrument: { ...request, instrumentJson: JSON.stringify(bad) },
    missingMarket: {
      ...request,
      marketJson: JSON.stringify({ ...market, curves: [] }),
    },
  };
}
