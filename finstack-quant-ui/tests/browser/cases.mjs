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
