// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { createRequire } from "node:module";
import type { OasResult, TrancheMetrics } from "finstack-quant-wasm";
import fixture from "./scenario-table/tranche-analytics.json";
import { budget } from "../shared/budget";
import oasConfigSchema from "../../src/generated/schemas/oas_config.json";
import { TrancheAnalytics } from "@/components/finstack/valuations/components/tranche-analytics/tranche-analytics";
import { TranchePanel } from "@/components/finstack/valuations/blocks/pricing-workbench/tranche-panel";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";

const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
type Case = (typeof fixture.cases)[number];
const byName = (name: string) =>
  fixture.cases.find((entry) => entry.name === name)! as Case & {
    results: Record<string, { value?: unknown; error?: { message: string } }>;
  };
const floating = byName("floating");
const oasDefault = oasConfigSchema.examples[0];
const fixed = byName("fixed");
let call: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, arg: any) => {
    if (method === "initialize") return { state: "ready", worker: true };
    if (method === "trancheMetricFieldUnits")
      return native.trancheMetricFieldUnits(arg);
    if (method === "trancheSolveFieldUnits")
      return native.trancheSolveFieldUnits();
    const deal = native.StructuredCredit.fromJson(arg.instrumentJson);
    try {
      const head = [deal, arg.trancheId, arg.marketJson, arg.asOf] as const;
      switch (method) {
        case "trancheMetrics":
          return native.trancheMetrics(
            ...head,
            arg.marketPricePct,
            arg.metrics,
          );
        case "trancheBreakevenCdr":
          return native.trancheBreakevenCdr(...head);
        case "trancheDiscountMargin":
          return native.trancheDiscountMargin(...head, arg.marketPricePct);
        case "trancheOas":
          return native.trancheOas(...head, arg.marketPricePct, arg.configJson);
        default:
          throw new Error(`Unexpected worker call: ${method}`);
      }
    } finally {
      deal.free();
    }
  });
  mock.createClient.mockReset().mockReturnValue({ call, close: vi.fn() });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const calls = (method: string) =>
  call.mock.calls.filter(([name]) => name === method).map(([, arg]) => arg);
const request = (entry: Case) => ({
  instrumentJson: JSON.stringify(entry.instrument),
  marketJson: JSON.stringify(entry.market),
  asOf: entry.asOf,
  trancheId: entry.trancheId,
});

it("shows native TrancheMetrics fields in Rust order with their documented units, then the requested registry metrics raw", () => {
  const metrics = floating.results.metrics.value as TrancheMetrics;
  render(
    <TrancheAnalytics
      trancheId="SENIOR"
      price="25"
      metrics={{ value: metrics }}
      metricUnits={{ value: native.trancheMetricFieldUnits(metrics.currency) }}
      solveUnits={{ value: native.trancheSolveFieldUnits() }}
      breakevenCdr={{ value: floating.results.breakevenCdr.value as number }}
      discountMargin={{
        value: floating.results.discountMargin.value as number,
      }}
      oas={{ value: floating.results.oas.value as OasResult }}
    />,
  );
  const table = screen.getByRole("table", { name: "SENIOR tranche metrics" });
  const rows = within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => [
      within(row).getByRole("rowheader").textContent,
      row.querySelector("td")!.textContent,
    ]);
  expect(rows.map(([label]) => label)).toEqual([
    "PV",
    "Clean price pct",
    "Dirty price pct",
    "Accrued",
    "Factor",
    "WAL",
    "YTM",
    "Z spread bp",
    "CS01",
    "Spread duration",
    "Spread convexity",
    "Effective duration",
    "Effective convexity",
    "Target price pct",
    "Dm bp",
    "Default01",
    "DV01",
    "Prepayment01",
    "Severity01",
  ]);
  const text = Object.fromEntries(rows);
  // Values are the returned digits; units are the Rust field docs' units.
  expect(text["Factor"]).toBe(`${metrics.factor}current / original balance`);
  expect(text["Target price pct"]).toBe("25% of current balance");
  expect(text["Dm bp"]).toContain("bp");
  expect(text["PV"]).toContain("USD");
  expect(text["DV01"]).toBe(`${metrics.metrics!.dv01}Unit unavailable`);
  const dm = screen.getByRole("region", { name: "Discount margin" });
  expect(dm.textContent).toContain(
    String(floating.results.discountMargin.value),
  );
  expect(dm.textContent).toContain("decimal");
  const oas = screen.getByRole("region", { name: "Option-adjusted spread" });
  expect(oas.textContent).toContain(
    String((floating.results.oas.value as OasResult).oas),
  );
  // A host-supplied OAS needs no solve control.
  expect(within(oas).queryByRole("button")).toBeNull();
});

it("shows each tile's own native error without hiding the others", () => {
  render(
    <TrancheAnalytics
      trancheId="CLONOTES-A"
      price="100"
      metrics={{ value: fixed.results.metrics.value as TrancheMetrics }}
      metricUnits={{ value: native.trancheMetricFieldUnits("USD") }}
      solveUnits={{ value: native.trancheSolveFieldUnits() }}
      breakevenCdr={{ value: fixed.results.breakevenCdr.value as number }}
      discountMargin={{ error: fixed.results.discountMargin.error!.message }}
      oas={{ value: fixed.results.oas.value as OasResult }}
    />,
  );
  expect(
    within(screen.getByRole("region", { name: "Discount margin" })).getByRole(
      "alert",
    ).textContent,
  ).toContain("only defined for floating-rate tranches");
  expect(
    screen.getByRole("region", { name: "Break-even CDR" }).textContent,
  ).toContain("0.00006103515625");
  // Fixed-rate notes have no discount margin field, so no row is shown.
  expect(
    within(
      screen.getByRole("table", { name: "CLONOTES-A tranche metrics" }),
    ).queryByRole("rowheader", { name: "Dm bp" }),
  ).toBeNull();
});

it("assumes no price: metrics use Rust's target, discount margin waits for a price and OAS runs only on request", async () => {
  render(
    <FinstackQueryProvider>
      <TranchePanel request={request(fixed)} metrics={fixed.metrics} />
    </FinstackQueryProvider>,
  );
  const table = await screen.findByRole("table", {
    name: "CLONOTES-A tranche metrics",
  });
  expect(calls("trancheMetrics")[0]).toEqual({
    ...request(fixed),
    marketPricePct: null,
    metrics: fixed.metrics,
  });
  expect(calls("trancheBreakevenCdr")).toHaveLength(1);
  expect(calls("trancheDiscountMargin")).toHaveLength(0);
  expect(calls("trancheOas")).toHaveLength(0);
  expect(
    screen.getByRole("region", { name: "Discount margin" }).textContent,
  ).toContain("Enter a clean price");
  const solve = screen.getByRole("button", { name: "Solve OAS" });
  expect((solve as HTMLButtonElement).disabled).toBe(true);
  expect(
    within(table).getByRole("rowheader", { name: "Target price pct" }),
  ).toBeTruthy();

  fireEvent.change(screen.getByLabelText("Clean price"), {
    target: { value: "100" },
  });
  // The fixed-rate note's discount margin fails natively in its own tile.
  await waitFor(() =>
    expect(
      within(screen.getByRole("region", { name: "Discount margin" })).getByRole(
        "alert",
      ).textContent,
    ).toBe(fixed.results.discountMargin.error!.message),
  );
  expect(calls("trancheDiscountMargin").at(-1).marketPricePct).toBe(100);
  expect(calls("trancheMetrics").at(-1).marketPricePct).toBe(100);
  expect(calls("trancheOas")).toHaveLength(0);
  expect(
    screen.getByRole("region", { name: "Option-adjusted spread" }).textContent,
  ).toContain("OAS runs on request");

  fireEvent.click(screen.getByRole("button", { name: "Solve OAS" }));
  const oas = screen.getByRole("region", { name: "Option-adjusted spread" });
  await waitFor(() =>
    expect(oas.textContent).toContain(
      String((fixed.results.oas.value as OasResult).oas),
    ),
  );
  expect(calls("trancheOas")).toEqual([
    { ...request(fixed), marketPricePct: 100 },
  ]);
  // A new price hides the solved OAS until it is requested again.
  fireEvent.change(screen.getByLabelText("Clean price"), {
    target: { value: "99" },
  });
  await waitFor(() => expect(oas.textContent).toContain("OAS runs on request"));
  expect(calls("trancheOas")).toHaveLength(1);
});

it("solves OAS under an applied OasConfig that starts from Rust's default", async () => {
  render(
    <FinstackQueryProvider>
      <TranchePanel request={request(fixed)} metrics={fixed.metrics} />
    </FinstackQueryProvider>,
  );
  await screen.findByRole("table", { name: "CLONOTES-A tranche metrics" });
  fireEvent.change(screen.getByLabelText("Clean price"), {
    target: { value: "100" },
  });
  const oas = screen.getByRole("region", { name: "Option-adjusted spread" });
  // Untouched, the solve omits the configuration and Rust uses its default.
  fireEvent.click(within(oas).getByRole("button", { name: "Solve OAS" }));
  await within(oas).findByText(
    "Num paths",
    { selector: "dt" },
    { timeout: budget(10000) },
  );
  expect(calls("trancheOas")).toEqual([
    { ...request(fixed), marketPricePct: 100 },
  ]);
  const paths = within(oas).getByLabelText("Num paths") as HTMLInputElement;
  // The form shows the schema's Rust example, OasConfig::default().
  expect(paths.value).toBe("256");
  fireEvent.change(paths, { target: { value: "8" } });
  const apply = within(oas).getByRole("button", {
    name: "Apply configuration",
  }) as HTMLButtonElement;
  // Apply waits for the (structural) validation of the edit to settle.
  await waitFor(() => expect(apply.disabled).toBe(false), {
    timeout: budget(5000),
  });
  fireEvent.click(apply);
  // An applied configuration hides the default-config result and runs nothing.
  await waitFor(() => expect(oas.textContent).toContain("OAS runs on request"));
  expect(calls("trancheOas")).toHaveLength(1);
  fireEvent.click(within(oas).getByRole("button", { name: "Solve OAS" }));
  await waitFor(() => expect(calls("trancheOas")).toHaveLength(2));
  const sent = calls("trancheOas")[1];
  expect(sent.marketPricePct).toBe(100);
  expect(JSON.parse(sent.configJson)).toEqual({
    ...(oasDefault as Record<string, unknown>),
    num_paths: 8,
  });
  const solved = await within(oas).findByText(
    "Num paths",
    { selector: "dt" },
    { timeout: budget(10000) },
  );
  expect(solved.nextElementSibling!.textContent).toContain("8");
});
