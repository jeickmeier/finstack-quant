// @vitest-environment jsdom
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import { PricingWorkbench } from "../registry/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "../registry/hooks/use-finstack/use-finstack";
import { serializeHost } from "../src/codec.mjs";
import detailFixtures from "./details/cases.json";
import cashflowFixtures from "./cashflows/cases.json";
import scenarioCases from "./scenarios/cases.json";
import pricingMarket from "./instruments/pricing-market.json";
import fixture from "../src/fixtures/results/bond.json";
import {
  type PriceRequest,
  FinstackError,
  errorValue,
} from "../registry/workers/finstack-contract";
import { formatMoney } from "../src/format/format";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("../registry/hooks/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
// jsdom has no Canvas/ResizeObserver. The installed Chromium test owns figure rendering.
vi.mock(
  "../registry/primitives/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("../registry/primitives/finstack-chart/finstack-chart")
    >()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let call: ReturnType<typeof vi.fn>, close: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, request: any) => {
    switch (method) {
      case "initialize":
        return { state: "ready", worker: true };
      case "models":
        return native.listModelsGrouped();
      case "metrics":
        return native.listStandardMetricsGrouped();
      case "metricMetadata":
        return native.metricMetadata(request);
      case "validate":
        return native.validateInstrumentJson(request);
      case "validateMarket": {
        const market = new native.Market(request);
        try {
          return market.toJson();
        } finally {
          market.free();
        }
      }
      case "validateCalibration":
        return native.validateCalibrationJson(request);
      case "dryRun":
        return native.dryRun(request);
      case "calibrate":
        return native.calibrate(request);
      case "scenarioTable":
        return native.structuredCreditTrancheScenarioTable(
          request.instrumentJson,
          request.trancheId,
          request.marketJson,
          request.asOf,
          request.gridJson,
        );
      case "formatMoney":
        return formatMoney(request.value, request.rounding, native);
      case "cashflows":
        return native.instrumentCashflowsJson(
          request.instrumentJson,
          request.marketJson,
          request.asOf,
          request.model,
        );
      case "price":
        try {
          return native.priceInstrument(
            request.instrumentJson,
            request.marketJson,
            request.asOf,
            request.model,
            request.metrics,
            request.pricingOptions,
            request.marketHistory,
          );
        } catch (error) {
          throw new FinstackError(errorValue(error));
        }
    }
  });
  close = vi.fn();
  mock.createClient.mockReset().mockReturnValue({ call, close });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const canonicalMarket = (json: string) => {
  const market = new native.Market(json);
  try {
    return market.toJson();
  } finally {
    market.free();
  }
};
function showOriginal(viewer: HTMLElement) {
  const button = within(viewer).queryByRole("button", {
    name: "Original",
    hidden: true,
  });
  if (button) fireEvent.click(button);
}
const prices = () =>
  call.mock.calls
    .filter(([method]) => method === "price")
    .map(([, request]) => request as PriceRequest);
function mount(request: PriceRequest = fixture.request) {
  return render(
    <div>
      <h1>Host route</h1>
      <FinstackQueryProvider>
        <PricingWorkbench defaultRequest={request} />
      </FinstackQueryProvider>
    </div>,
  );
}
it("embeds the existing components, debounces complete requests and retains edits/results across tabs", async () => {
  const view = mount();
  await waitFor(() => expect(prices()).toHaveLength(1), { timeout: 3000 });
  expect(prices()[0]).toEqual({
    ...fixture.request,
    marketJson: canonicalMarket(fixture.request.marketJson),
    instrumentJson: native.validateInstrumentJson(
      fixture.request.instrumentJson,
    ),
  });
  const results = screen.getByRole("region", { name: "Results" });
  expect(
    (await within(results).findByTitle(fixture.result.value.amount))
      .textContent,
  ).toBe("USD 1,042,500");
  const amount = screen.getByRole("textbox", { name: "Amount" });
  fireEvent.change(amount, { target: { value: "1000000.123456789" } });
  await userEvent.click(screen.getByRole("button", { name: "Settings" }));
  const overrides = ' {"theta_period":"1W"} ';
  const history = '{"base_date":"2025-01-01","window_days":2,"scenarios":[]}';
  fireEvent.change(screen.getByRole("textbox", { name: "Pricing overrides" }), {
    target: { value: overrides },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Market history" }), {
    target: { value: history },
  });
  await waitFor(() => expect(prices()).toHaveLength(2), { timeout: 3000 });
  expect(prices()[1]).toEqual({
    ...prices()[0],
    instrumentJson: expect.stringContaining('"amount":"1000000.123456789"'),
    pricingOptions: overrides,
    marketHistory: history,
  });
  await userEvent.click(screen.getByRole("tab", { name: "2 Market" }));
  await userEvent.click(screen.getByRole("tab", { name: "Snapshot JSON" }));
  showOriginal(screen.getByRole("region", { name: "Market snapshot" }));
  const snapshot = screen.getByRole("region", { name: "Market snapshot" });
  expect(snapshot.querySelector("pre")!.textContent).toBe(
    canonicalMarket(fixture.request.marketJson),
  );
  expect(snapshot.querySelector("[data-json-print-source]")!.textContent).toBe(
    canonicalMarket(fixture.request.marketJson),
  );
  expect(screen.getByRole("region", { name: "Results" })).toBe(results);
  await userEvent.click(screen.getByRole("tab", { name: "1 Instrument" }));
  expect(
    (screen.getByRole("textbox", { name: "Amount" }) as HTMLInputElement).value,
  ).toBe("1000000.123456789");
  expect(mock.createClient).toHaveBeenCalledTimes(1);
  view.unmount();
  expect(close).toHaveBeenCalledTimes(1);
}, 15000);
it("retains the completed context during invalid edits, native pricing failure and catalogue selection", async () => {
  mount();
  expect(
    (
      await screen.findByTitle(
        fixture.result.value.amount,
        {},
        { timeout: 3000 },
      )
    ).textContent,
  ).toBe("USD 1,042,500");
  const amount = screen.getByRole("textbox", { name: "Amount" });
  fireEvent.change(amount, { target: { value: "abc" } });
  const issue = await screen.findByRole("button", {
    name: "instrument.spec.notional.amount",
  });
  await userEvent.click(issue);
  expect(document.activeElement).toBe(amount);
  expect(prices()).toHaveLength(1);
  expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
    "USD 1,042,500",
  );
  fireEvent.change(amount, { target: { value: "1000000" } });
  await userEvent.click(screen.getByRole("button", { name: "Settings" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Pricing overrides" }), {
    target: { value: '{"unknown_seed":18446744073709551615}' },
  });
  await waitFor(
    () =>
      expect(
        screen
          .getAllByRole("alert")
          .some((node) => node.textContent?.includes("unknown_seed")),
      ).toBe(true),
    { timeout: 3000 },
  );
  expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
    "USD 1,042,500",
  );
  await userEvent.click(
    screen.getByRole("combobox", { name: "Instrument type" }),
  );
  await userEvent.click(screen.getByRole("option", { name: "equity" }));
  expect(
    await screen.findByRole("button", { name: "Load example" }),
  ).toBeTruthy();
  await waitFor(() => expect(prices()).toHaveLength(3), { timeout: 3000 });
  expect(prices()[2].model).toBe(prices()[0].model);
  expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
    "USD 1,042,500",
  );
}, 15000);
it("rejects a calibration envelope as a market snapshot before issuing a price request", async () => {
  mount({
    ...fixture.request,
    marketJson: '{"schema_version":1,"calibration":{"type":"discount"}}',
  });
  const marketTab = screen.getByRole("tab", { name: "2 Market" });
  await userEvent.click(marketTab);
  await userEvent.click(screen.getByRole("tab", { name: "Edit market" }));
  expect(
    within(
      screen.getByRole("region", { name: "Market context form" }),
    ).getByRole("alert"),
  ).toBeTruthy();
  await waitFor(() =>
    expect(call.mock.calls.some(([method]) => method === "validate")).toBe(
      true,
    ),
  );
  expect(prices()).toEqual([]);
});

it("feeds the complete native-validated market edit to pricing and shares stored-node selection", async () => {
  mount();
  expect(
    (
      await screen.findByTitle(
        fixture.result.value.amount,
        {},
        { timeout: 3000 },
      )
    ).textContent,
  ).toBe("USD 1,042,500");
  await userEvent.click(screen.getByRole("tab", { name: "2 Market" }));
  await userEvent.click(screen.getByRole("tab", { name: "Edit market" }));
  fireEvent.change(screen.getAllByLabelText("Stored value 1")[0], {
    target: { value: "0.96" },
  });
  await waitFor(() => expect(prices()).toHaveLength(2), { timeout: 4000 });
  const request = prices()[1];
  const expected = JSON.parse(fixture.request.marketJson);
  expected.curves[0].knot_points[1][1] = 0.96;
  expect(request.marketJson).toBe(canonicalMarket(JSON.stringify(expected)));
  expect({ ...request, marketJson: prices()[0].marketJson }).toEqual(
    prices()[0],
  );
  await userEvent.click(screen.getByRole("tab", { name: "View market" }));
  const curveIndex = JSON.parse(request.marketJson).curves.findIndex(
    (curve: any) => curve.id === expected.curves[0].id,
  );
  fireEvent.change(screen.getByLabelText("Search market fields"), {
    target: { value: `/curves/${curveIndex}` },
  });
  fireEvent.click(
    screen.getByRole("button", { name: `Inspect /curves/${curveIndex}` }),
  );
  const cell = screen.getByRole("cell", { name: "0.96" });
  await userEvent.click(cell);
  expect(cell.getAttribute("aria-selected")).toBe("true");
  await userEvent.click(screen.getByRole("tab", { name: "4 Diagnostics" }));
  const direct = native.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    request.asOf,
    request.model,
    request.metrics,
  );
  await waitFor(() =>
    expect(
      screen
        .getByRole("region", {
          name: "Complete valuation result JSON",
          hidden: true,
        })
        .querySelector("pre")!.textContent,
    ).toContain(direct.value.amount),
  );
}, 15000);

it.each(detailFixtures.cases)(
  "composes actual $type pricing with its returned $detailType route",
  async (entry) => {
    mount(entry.request);
    await userEvent.click(screen.getByRole("tab", { name: "4 Diagnostics" }));
    const raw = await screen.findByRole(
      "region",
      { name: "Complete valuation result JSON", hidden: true },
      { timeout: 10000 },
    );
    showOriginal(raw);
    const result = JSON.parse(raw.querySelector("pre")!.textContent!);
    const index = call.mock.calls.findLastIndex(
      ([method]) => method === "price",
    );
    expect(raw.querySelector("pre")!.textContent).toBe(
      serializeHost(await call.mock.results[index].value),
    );
    expect(result.details.type).toBe(entry.detailType);
    expect(JSON.parse(prices().at(-1)!.instrumentJson).instrument.type).toBe(
      entry.type,
    );
    if (entry.detailType === "monte_carlo")
      expect(
        await screen.findByRole("region", { name: "Monte Carlo diagnostics" }),
      ).toBeTruthy();
    else {
      const labels: Record<string, string> = {
        composite: "Composite details JSON",
        credit_derivative: "Credit derivative details JSON",
        fx: "FX details JSON",
        structured_credit_stochastic: "Structured credit details JSON",
      };
      const details = await screen.findByRole("region", {
        name: labels[entry.detailType],
      });
      expect(JSON.parse(details.querySelector("pre")!.textContent!)).toEqual(
        result.details.data,
      );
    }
    expect(raw.querySelector("pre")!.textContent).toContain('"meta":');
  },
  20000,
);
it.each(cashflowFixtures.cases.filter((entry) => entry.type !== "bond"))(
  "keeps $type pricing intact with exact cashflow output or its actual native export error",
  async (entry) => {
    mount(entry.request);
    const results = within(screen.getByRole("region", { name: "Results" }));
    const tabs = within(results.getByRole("tablist", { name: "Result views" }));
    await userEvent.click(tabs.getByRole("tab", { name: "4 Diagnostics" }));
    const result = await results.findByRole(
      "region",
      { name: "Complete valuation result JSON", hidden: true },
      { timeout: 10000 },
    );
    expect(JSON.parse(result.querySelector("pre")!.textContent!).value).toEqual(
      entry.pricedValue,
    );
    await userEvent.click(tabs.getByRole("tab", { name: "3 Cashflows" }));
    const viewer = await results.findByRole("region", { name: "Cashflows" });
    if (entry.error)
      await waitFor(() =>
        expect(within(viewer).getByRole("alert").textContent).toBe(
          `Cashflows unavailable: ${entry.error}`,
        ),
      );
    else {
      await within(viewer).findByRole("table", { name: "Cashflow schedule" });
      fireEvent.click(
        within(viewer).getByText("Original JSON", { exact: true }),
      );
      await waitFor(() => {
        showOriginal(viewer);
        expect(viewer.querySelector("pre")!.textContent).toBe(entry.cashflows);
      });
    }
    expect(JSON.parse(result.querySelector("pre")!.textContent!).value).toEqual(
      entry.pricedValue,
    );
    expect(prices()).toHaveLength(1);
    expect(prices()[0].model).toBe(entry.request.model);
  },
  20000,
);
it("lets the host deep link select another canonical example while preserving explicit market and parameters", async () => {
  render(
    <FinstackQueryProvider>
      <PricingWorkbench
        defaultRequest={fixture.request}
        defaultInstrumentType="equity"
      />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(prices()).toHaveLength(1), { timeout: 5000 });
  const request = prices()[0];
  expect(JSON.parse(request.instrumentJson).instrument.type).toBe("equity");
  expect(request.marketJson).toBe(canonicalMarket(fixture.request.marketJson));
  expect(request.model).toBe(fixture.request.model);
});

it("loads configured scenario prices only on demand from the completed structured-credit context", async () => {
  const request = {
    instrumentJson: JSON.stringify(scenarioCases.cases[0]!.instrument),
    marketJson: JSON.stringify(pricingMarket),
    asOf: scenarioCases.asOf,
    model: "discounting",
    metrics: [],
  };
  const scenario = {
    trancheId: scenarioCases.trancheId,
    gridJson: JSON.stringify(scenarioCases.grid),
    priceDomain: [80, 140] as const,
  };
  render(
    <FinstackQueryProvider>
      <PricingWorkbench defaultRequest={request} scenario={scenario} />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(prices()).toHaveLength(1), { timeout: 10000 });
  expect(call.mock.calls.some(([method]) => method === "scenarioTable")).toBe(
    false,
  );
  await userEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
  const toggle = await screen.findByText(
    "Scenario prices",
    { selector: "summary" },
    { timeout: 5000 },
  );
  fireEvent.click(toggle);
  await waitFor(
    () =>
      expect(
        call.mock.calls.some(([method]) => method === "scenarioTable"),
      ).toBe(true),
    { timeout: 5000 },
  );
  const invocation = call.mock.calls.find(
    ([method]) => method === "scenarioTable",
  )![1];
  expect(invocation).toEqual({
    instrumentJson: prices()[0].instrumentJson,
    marketJson: prices()[0].marketJson,
    asOf: prices()[0].asOf,
    trancheId: scenario.trancheId,
    gridJson: scenario.gridJson,
  });
  await waitFor(
    () =>
      expect(
        document.querySelector('[aria-label="CLONOTES-A scenario prices"]'),
      ).toBeTruthy(),
    { timeout: 5000 },
  );
  expect(prices()).toHaveLength(1);
}, 30000);

it("prepares one completed cashflow snapshot before printing and restores the input tab", async () => {
  const invoke = call.getMockImplementation()! as (
    method: string,
    request: any,
  ) => Promise<any>;
  let release!: () => void;
  const pending = new Promise<void>((resolve) => {
    release = resolve;
  });
  call.mockImplementation(async (method, request) => {
    if (method === "cashflows") await pending;
    return invoke(method, request);
  });
  const print = vi.fn();
  vi.stubGlobal("print", print);
  vi.stubGlobal("requestAnimationFrame", (callback: () => void) =>
    setTimeout(callback, 0),
  );
  Object.defineProperty(document, "fonts", {
    configurable: true,
    value: { ready: Promise.resolve() },
  });
  try {
    mount();
    const button = await screen.findByRole("button", { name: "Print report" });
    await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false), {
      timeout: 10000,
    });
    const request = prices()[0]!;
    fireEvent.click(button);
    const workbench = screen.getByRole("region", { name: "Pricing workbench" });
    await waitFor(() =>
      expect(workbench.getAttribute("data-printing")).toBe("preparing"),
    );
    expect(print).not.toHaveBeenCalled();
    expect(
      screen
        .getByRole("tab", { name: "2 Market" })
        .getAttribute("aria-selected"),
    ).toBe("true");
    release();
    await waitFor(() => expect(print).toHaveBeenCalledTimes(1));
    const cashflowCalls = call.mock.calls.filter(
      ([method]) => method === "cashflows",
    );
    expect(cashflowCalls).toHaveLength(1);
    expect(cashflowCalls[0]![1]).toEqual({
      instrumentJson: request.instrumentJson,
      marketJson: request.marketJson,
      asOf: request.asOf,
      model: request.model ?? "default",
    });
    showOriginal(
      workbench.querySelector('[aria-label="Priced instrument JSON"]')!,
    );
    showOriginal(workbench.querySelector('[aria-label="Market snapshot"]')!);
    expect(
      workbench.querySelector('[aria-label="Priced instrument JSON"] pre')
        ?.textContent,
    ).toBe(request.instrumentJson);
    expect(
      workbench.querySelector('[aria-label="Market snapshot"] pre')
        ?.textContent,
    ).toBe(request.marketJson);
    fireEvent(window, new Event("afterprint"));
    await waitFor(() =>
      expect(workbench.hasAttribute("data-printing")).toBe(false),
    );
    expect(
      screen
        .getByRole("tab", { name: "1 Instrument" })
        .getAttribute("aria-selected"),
    ).toBe("true");
    expect(
      screen
        .getByRole("tab", { name: "3 Cashflows" })
        .getAttribute("aria-selected"),
    ).toBe("true");
  } finally {
    Reflect.deleteProperty(document, "fonts");
    release();
  }
}, 30000);

it("keeps shortcuts within the focused workbench and leaves field typing intact", () => {
  const first = mount(),
    second = mount();
  const [left, right] = screen.getAllByRole("region", {
    name: "Pricing workbench",
  });
  fireEvent.keyDown(left, { key: "2" });
  expect(
    within(left)
      .getByRole("tab", { name: "2 Market" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  expect(
    within(right)
      .getByRole("tab", { name: "1 Instrument" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  fireEvent.keyDown(within(right).getByRole("textbox", { name: "As of" }), {
    key: "2",
  });
  expect(
    within(right)
      .getByRole("tab", { name: "1 Instrument" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  fireEvent.keyDown(left, { key: "4" });
  expect(
    within(left)
      .getByRole("tab", { name: "4 Diagnostics" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  expect(
    within(right)
      .getByRole("tab", { name: "3 Cashflows" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  first.unmount();
  second.unmount();
});
