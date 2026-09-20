// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { QueryClient } from "@tanstack/react-query";
import { createChartRuntime } from "@tanstack/charts";
import { MarketContextForm } from "../registry/components/market-context-form/market-context-form";
import {
  marketModule,
  importMarket,
} from "../registry/components/market-context-form/market";
import {
  curvePanels,
  curveDefinition,
  type CurveState,
  type CurvePoint,
} from "../registry/components/curve-chart/curve-chart";
import { priceOptions } from "../registry/hooks/use-price-instrument/use-price-instrument";
import type { FinstackClient } from "../registry/hooks/use-finstack/client";
import { errorValue, unwrap } from "../registry/workers/finstack-contract";
import { serializeHost } from "../src/codec.mjs";
import { startWorker } from "./worker/harness.mjs";
import bond from "../src/fixtures/results/bond.json";
import marketFixture from "./instruments/pricing-market.json";
import curves from "../src/fixtures/curves/market.json";
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, 30000);
afterAll(async () => harness?.close());
afterEach(cleanup);
const validate = async (json: string) =>
  unwrap(await harness.proxy.validateMarket(json));
function canonical(json: string) {
  const market = new native.Market(json);
  try {
    return market.toJson();
  } finally {
    market.free();
  }
}
async function submit(onSubmit: ReturnType<typeof vi.fn>) {
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Apply market",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByRole("button", { name: "Apply market" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalled());
  return onSubmit.mock.calls.at(-1)![0] as string;
}
it("edits a stored knot, redraws that exact coordinate and prices the full edited payload", async () => {
  const onSubmit = vi.fn();
  render(
    <MarketContextForm
      defaultJson={bond.request.marketJson}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  fireEvent.change(screen.getAllByLabelText("Stored value 1")[0], {
    target: { value: "0.96" },
  });
  const json = await submit(onSubmit);
  const expected = JSON.parse(bond.request.marketJson);
  expected.curves[0].knot_points[1][1] = 0.96;
  expect(JSON.parse(json)).toEqual(
    JSON.parse(canonical(JSON.stringify(expected))),
  );
  const panel = curvePanels(JSON.parse(json).curves as CurveState[]).find(
    (p) => p.type === "discount",
  )!;
  const runtime = createChartRuntime<CurvePoint, number, number>();
  try {
    const scene = runtime.render(
      curveDefinition(panel.points, panel.type, {}),
      { width: 800, height: 400 },
    );
    expect(scene.points.map((p) => [p.xValue, p.yValue])).toEqual(
      expected.curves[0].knot_points,
    );
  } finally {
    runtime.destroy();
  }
  const client = {
    call: async (_method: unknown, request: typeof bond.request) =>
      unwrap(await harness.proxy.price(request)),
  } as unknown as FinstackClient;
  const query = new QueryClient();
  const request = { ...bond.request, marketJson: json };
  try {
    const result = await query.fetchQuery(priceOptions(client, 1, request));
    const direct = native.priceInstrument(
      request.instrumentJson,
      json,
      request.asOf,
      request.model,
      request.metrics,
    );
    direct.meta.timestamp = result.meta.timestamp;
    expect(result).toEqual(direct);
    expect(result.value.amount).not.toBe(bond.result.value.amount);
  } finally {
    query.clear();
  }
});
it("edits no-knot parametric and base-correlation shapes and preserves every other market field", async () => {
  const onSubmit = vi.fn();
  const selected = structuredClone(curves);
  // Locate actual no-knot variants without inventing model fields.
  selected.curves = curves.curves.filter((c) => !("knot_points" in c));
  render(
    <MarketContextForm
      defaultJson={JSON.stringify(selected)}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  expect(screen.queryByLabelText("Stored value 0")).toBeNull();
  const input = document.querySelector(
    '[data-field-path="curves[0].correlations[1]"] input',
  ) as HTMLInputElement;
  expect(input).toBeTruthy();
  fireEvent.change(input, { target: { value: "0.31" } });
  const parametric = document.querySelector(
    '[data-field-path="curves[1].model.beta0"] input',
  ) as HTMLInputElement;
  expect(parametric).toBeTruthy();
  fireEvent.change(parametric, { target: { value: "0.025" } });
  const json = await submit(onSubmit);
  const expected = structuredClone(selected) as unknown as {
    curves: Record<string, unknown>[];
  };
  (expected.curves[0].correlations as number[])[1] = 0.31;
  (expected.curves[1].model as { beta0: number }).beta0 = 0.025;
  expect(JSON.parse(json)).toEqual(
    JSON.parse(canonical(JSON.stringify(expected))),
  );
  expect(
    JSON.parse(json).curves.every((c: object) => !("knot_points" in c)),
  ).toBe(true);
});
it("imports an actual native calibration envelope and canonical fixture, leaving other fields read-only", async () => {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const envelope = native.calibrate(
    readFileSync(
      resolve(
        root,
        "finstack-quant/calibration/examples/market_bootstrap/01_usd_discount.json",
      ),
      "utf8",
    ),
  );
  const imported = importMarket(serializeHost(envelope));
  expect(imported).toEqual(
    marketModule.codec.parse(
      canonical(serializeHost(envelope.result.final_market)),
    ),
  );
  const onSubmit = vi.fn();
  render(
    <MarketContextForm
      defaultJson={bond.request.marketJson}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  const readonly = screen.getByRole("region", {
    name: "Read-only market data",
    hidden: true,
  });
  expect(readonly.querySelector("pre")?.textContent).toContain('"vol_cubes"');
  expect(
    document.querySelector('[data-field-path="vol_cubes"] input'),
  ).toBeNull();
  fireEvent.change(screen.getByLabelText("Market or calibration result JSON"), {
    target: { value: serializeHost(envelope) },
  });
  fireEvent.click(screen.getByText("Import JSON"));
  expect(JSON.parse(await submit(onSubmit))).toEqual(
    JSON.parse(canonical(serializeHost(envelope.result.final_market))),
  );
  onSubmit.mockClear();
  fireEvent.click(screen.getByText("Load bond market example"));
  expect(await submit(onSubmit)).toBe(canonical(bond.request.marketJson));
});
it("retains actual native domain errors and frees every successful validation handle", async () => {
  const local = await startWorker();
  try {
    for (let i = 0; i < 3; i++)
      expect(
        unwrap(await local.proxy.validateMarket(bond.request.marketJson)),
      ).toBe(canonical(bond.request.marketJson));
    const resources = await (
      local.proxy as unknown as {
        resources(): Promise<{ constructed: number; freed: number }>;
      }
    ).resources();
    expect(resources).toMatchObject({ constructed: 3, freed: 3 });
    const invalid = JSON.parse(bond.request.marketJson);
    invalid.curves[0].knot_points[1][1] = -1;
    let direct;
    try {
      canonical(JSON.stringify(invalid));
    } catch (error) {
      direct = errorValue(error);
    }
    expect(direct).toBeTruthy();
    expect(await local.proxy.validateMarket(JSON.stringify(invalid))).toEqual({
      ok: false,
      error: direct,
    });
    const onSubmit = vi.fn();
    render(
      <MarketContextForm
        defaultJson={JSON.stringify(invalid)}
        validate={validate}
        onSubmit={onSubmit}
      />,
    );
    await waitFor(() =>
      expect(document.body.textContent).toContain(direct!.message),
    );
    expect(onSubmit).not.toHaveBeenCalled();
  } finally {
    await local.close();
  }
});
it("rejects malformed imports without discarding edits or exposing readonly data as controls", async () => {
  const onSubmit = vi.fn();
  render(
    <MarketContextForm
      defaultJson={bond.request.marketJson}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  fireEvent.change(screen.getAllByLabelText("Stored value 1")[0], {
    target: { value: "0.97" },
  });
  fireEvent.change(screen.getByLabelText("Market or calibration result JSON"), {
    target: { value: "{bad" },
  });
  fireEvent.click(screen.getByText("Import JSON"));
  expect(screen.getAllByRole("alert").length).toBeGreaterThan(0);
  expect(
    (screen.getAllByLabelText("Stored value 1")[0] as HTMLInputElement).value,
  ).toBe("0.97");
  expect(
    JSON.parse(await submit(onSubmit)).curves.find(
      (c: { id: string }) => c.id === "USD-OIS",
    ).knot_points[1][1],
  ).toBe(0.97);
});

it("edits stored FX entries and scalar prices while preserving populated read-only surfaces", async () => {
  const market = {
    ...marketModule.example,
    fx: marketFixture.fx,
    prices: { "SPX-SPOT": marketFixture.prices["SPX-SPOT"] },
    surfaces: marketFixture.surfaces,
    // Values from core/tests/market_data/serde.rs full-state fixture; generated wire shapes.
    vol_cubes: [
      {
        id: "USD-SWAPTION",
        expiries: [1],
        tenors: [5],
        params: [{ alpha: 0.035, beta: 0.5, rho: -0.2, nu: 0.4, shift: null }],
        forwards: [0.03],
        interpolation_mode: "vol",
      },
    ],
    fx_delta_vol_surfaces: [
      {
        id: "EURUSD-DELTA-VOL",
        expiries: [1],
        atm_vols: [0.08],
        rr_25d: [0.01],
        bf_25d: [0.005],
        rr_10d: null,
        bf_10d: null,
      },
    ],
  };
  const onSubmit = vi.fn();
  render(
    <MarketContextForm
      defaultJson={JSON.stringify(market)}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  const input = (path: string) =>
    document.querySelector(
      `[data-field-path="${path}"] input`,
    ) as HTMLInputElement;
  expect(input("fx.quotes[0][2]")).toBeTruthy();
  fireEvent.change(input("fx.quotes[0][2]"), { target: { value: "1.12" } });
  expect(input("prices[0].value.unitless")).toBeTruthy();
  fireEvent.change(input("prices[0].value.unitless"), {
    target: { value: "4600" },
  });
  const json = await submit(onSubmit);
  const expected = structuredClone(market);
  expected.fx.quotes[0][2] = 1.12;
  expected.prices["SPX-SPOT"].unitless = 4600;
  expect(JSON.parse(json)).toEqual(
    JSON.parse(canonical(JSON.stringify(expected))),
  );
  const result = JSON.parse(json);
  for (const key of [
    "surfaces",
    "vol_cubes",
    "fx_delta_vol_surfaces",
  ] as const) {
    expect(result[key]).toEqual(
      JSON.parse(canonical(JSON.stringify(market)))[key],
    );
    expect(input(key)).toBeNull();
  }
});
