import { expect, it } from "vitest";
import { z } from "zod";
import { readFile } from "node:fs/promises";
import { getCurveView, getValueView } from "../src/views";
import { converterSchema } from "../src/schema.mjs";
import curves from "../src/generated/curve-views.json";

const market = JSON.parse(
  await readFile(
    new URL(
      "../../finstack-quant/core/schemas/market_data/1/market_context_state.schema.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const variants = market.$defs.CurveState.oneOf;
const values: Record<string, unknown> = {
  id: "STORED",
  base: "2025-01-01",
  base_date: "2025-01-01",
  day_count: "act_365f",
  interp_style: "linear",
  extrapolation: "flat_forward",
  knot_points: [
    [0.25, 0.987654321],
    [2, 0.92],
  ],
  allow_non_monotonic: false,
  min_forward_tenor: 1e-6,
  reset_lag: 2,
  tenor: 0.25,
  recovery_rate: 0.4,
  par_points: [[1, 0.01]],
  base_cpi: 100,
  detachment_points: [0.03, 0.07],
  correlations: [0.2, 0.3],
  model: { variant: "ns", beta0: 0.03, beta1: -0.01, beta2: 0.005, tau: 2 },
};

it("covers every canonical curve tag without a synthetic no-knot route", () => {
  expect(curves.map((curve) => curve.type)).toEqual(
    variants.map(
      (variant: { properties: { type: { const: string } } }) =>
        variant.properties.type.const,
    ),
  );
  expect(
    curves
      .filter((curve) => curve.route === "fields")
      .map((curve) => curve.type),
  ).toEqual(["base_correlation", "parametric"]);
});
for (const variant of variants) {
  it(`preserves schema-validated ${variant.properties.type.const} state`, () => {
    const state = Object.fromEntries(
      variant.required.map((key: string) => [
        key,
        key === "type" ? variant.properties.type.const : values[key],
      ]),
    );
    const validator = z.fromJSONSchema(
      converterSchema({ ...variant, $defs: market.$defs }),
    );
    expect(validator.safeParse(state).success).toBe(true);
    const before = structuredClone(state);
    const view = getCurveView(state);
    expect(view.state).toBe(state);
    if (view.kind === "knots") expect(view.points).toBe(state.knot_points);
    else {
      expect(view.fields).toEqual(state);
      expect(view).not.toHaveProperty("points");
    }
    expect(state).toEqual(before);
  });
}
it("retains raw values and source help when financial units are unavailable", () => {
  const raw = {
    amount: "123.456789012345678901",
    currency: "EUR",
    seed: 18446744073709551615n,
  };
  const view = getValueView(raw, { description: "Stored source description" });
  expect(view.value).toBe(raw);
  expect(view.unitStatus).toBe("unavailable");
  expect(view.unit).toBeNull();
  expect(view.description).toBe("Stored source description");
  expect(getValueView("0.025", { "x-unit": "source-unit" })).toMatchObject({
    value: "0.025",
    unit: "source-unit",
    unitStatus: "source",
  });
  expect(() => getCurveView({ type: "unknown" })).toThrow();
  expect(() => getCurveView({ type: "discount" })).toThrow();
});
