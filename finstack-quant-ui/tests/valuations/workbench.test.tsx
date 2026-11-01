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
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import { serializeHost } from "../../src/codec.mjs";
import detailFixtures from "./details/cases.json";
import cashflowFixtures from "./cashflows/cases.json";
import scenarioCases from "./scenario-table/cases.json";
import standardDeals from "./scenario-table/standard-deals.json";
import scenarioGridSchema from "../../src/generated/schemas/scenario_grid.json";
import pricingMarket from "./instruments/pricing-market.json";
import type { ValuationResult } from "finstack-quant-wasm";
import bondFixture from "../../src/fixtures/results/bond.json";
/** The JSON import widens currency codes to `string`; the fixture is a native result. */
const fixture = bondFixture as typeof bondFixture & { result: ValuationResult };
import {
  type PriceRequest,
  FinstackError,
  errorValue,
  unwrap,
} from "@/workers/finstack-contract";
import { formatMoney, formatRawMoney } from "../../src/format/format";
import { budget } from "../shared/budget";
import { createContext } from "@/workers/services/context";
import { createScenariosService } from "@/workers/services/scenarios";
import { createAttributionService } from "@/workers/services/attribution";
import { createModelsService } from "@/workers/services/models";
import calibrationCases from "../calibration/cases.json";
import marketScenarios from "../scenarios/application/cases.json";
const bondDisplay = formatRawMoney(fixture.result.value);
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
// jsdom has no Canvas/ResizeObserver. The installed Chromium test owns figure rendering.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("@/components/finstack/shared/chart/finstack-chart/finstack-chart")
    >()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let call: ReturnType<typeof vi.fn>, close: ReturnType<typeof vi.fn>;
// The production ScenariosApi over the Node facade; its envelopes unwrap as the client does.
const scenarioService = createScenariosService(
  { scenarios: native, attribution: native },
  createContext({
    initialize: async () => undefined,
    core: { MarketContext: native.MarketContext },
    portfolio: { Portfolio: native.Portfolio },
  }),
);
// The production AttributionApi over the Node facade.
const attributionService = createAttributionService(
  { attribution: native },
  createContext({
    initialize: async () => undefined,
    core: { MarketContext: native.MarketContext },
    portfolio: { Portfolio: native.Portfolio },
  }),
);
// The production checkSurface over the Node facade's VolSurface and checkSurface.
const modelsService = createModelsService(
  {
    core: { VolSurface: native.VolSurface },
    models: {
      volatility: { checkSurface: native.checkSurface },
      credit: { migration: {} },
    },
  } as unknown as Parameters<typeof createModelsService>[0],
  createContext({
    initialize: async () => undefined,
    core: { MarketContext: native.MarketContext },
    portfolio: { Portfolio: native.Portfolio },
  }),
);
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  call = vi.fn(async (method: string, request: any) => {
    switch (method) {
      case "initialize":
        return { state: "ready", worker: true };
      case "models":
        return native.listModelsGrouped();
      case "metrics":
        return native.metricsForInstrument(request);
      case "metricMetadata":
        return native.metricMetadata(request);
      case "validate":
        return native.validateInstrumentJson(request);
      case "validateMarket": {
        const market = native.MarketContext.fromJson(request);
        try {
          return market.toJson();
        } finally {
          market.free();
        }
      }
      case "validateCalibration":
        return native.validateCalibration(request);
      case "dryRun":
        return native.dryRun(request);
      case "calibrate":
        return native.calibrate(request);
      case "newStandardDeal": {
        const deal = native.StructuredCredit.newStandard(
          request.dealType,
          request.paramsJson,
        );
        try {
          return deal.toJson();
        } finally {
          deal.free();
        }
      }
      case "dealTranches": {
        const deal = native.StructuredCredit.fromJson(request);
        try {
          return deal.tranches;
        } finally {
          deal.free();
        }
      }
      case "trancheSchedule": {
        const deal = native.StructuredCredit.fromJson(request.instrumentJson);
        try {
          const cashflows = deal.trancheCashflows(
            request.trancheId,
            request.marketJson,
            request.asOf,
          );
          return {
            cashflows,
            table: native.trancheCashflowsTable(cashflows),
          };
        } finally {
          deal.free();
        }
      }
      case "poolReport": {
        const deal = native.StructuredCredit.fromJson(request.instrumentJson);
        try {
          return native.poolReport(deal.pool, request.asOf);
        } finally {
          deal.free();
        }
      }
      case "poolReportFieldUnits":
        return native.poolReportFieldUnits(request);
      case "runSimulation": {
        const deal = native.StructuredCredit.fromJson(request.instrumentJson);
        try {
          return deal.runSimulation(request.marketJson, request.asOf);
        } finally {
          deal.free();
        }
      }
      case "equityMetrics": {
        const deal = native.StructuredCredit.fromJson(request.instrumentJson);
        try {
          return deal.equityMetrics(
            request.marketJson,
            request.asOf,
            request.purchasePricePct,
          );
        } finally {
          deal.free();
        }
      }
      case "equityMetricsFieldUnits":
        return native.equityMetricsFieldUnits(request);
      case "scenarioTableFieldUnits":
        return native.scenarioTableFieldUnits(request);
      case "simulationFieldUnits":
        return native.simulationFieldUnits(request);
      case "scenarioTable": {
        const deal = native.StructuredCredit.fromJson(request.instrumentJson);
        try {
          return native.trancheScenarioTable(
            deal,
            request.trancheId,
            request.marketJson,
            request.asOf,
            request.gridJson,
          );
        } finally {
          deal.free();
        }
      }
      case "buildMarketScenario":
        return unwrap(await scenarioService.buildMarketScenario(request));
      case "applyMarketScenario":
        return unwrap(await scenarioService.applyMarketScenario(request));
      case "horizonReturn":
        return unwrap(await scenarioService.horizonReturn(request));
      case "validateScenarioSpec":
        return unwrap(await scenarioService.validateScenarioSpec(request));
      case "listBuiltinTemplateMetadata":
        return unwrap(await scenarioService.listBuiltinTemplateMetadata());
      case "attributePnlMany":
        return unwrap(await attributionService.attributePnlMany(request));
      case "checkSurface":
        return unwrap(await modelsService.checkSurface(request));
      case "defaultWaterfallOrder":
        return native.defaultWaterfallOrder();
      case "calendars":
        return native.availableCalendars();
      case "formatMoney":
        return formatMoney(request.value, request.rounding, native);
      case "cashflows":
        return native.instrumentCashflows(
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
            request.metricPricingOverrides,
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
  const market = native.MarketContext.fromJson(json);
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
it(
  "embeds the existing components, debounces complete requests and retains edits/results across tabs",
  async () => {
    const view = mount();
    await waitFor(() => expect(prices()).toHaveLength(1), {
      timeout: budget(3000),
    });
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
    ).toBe(bondDisplay);
    const amount = screen.getByRole("textbox", { name: "Amount" });
    fireEvent.change(amount, { target: { value: "1000000.123456789" } });
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    const history = '{"base_date":"2025-01-01","window_days":2,"scenarios":[]}';
    await userEvent.click(
      screen.getByRole("button", { name: "Add theta period" }),
    );
    fireEvent.change(
      within(screen.getByText("Pricing overrides").parentElement!).getByRole(
        "textbox",
        { name: "Count" },
      ),
      {
        target: { value: "1" },
      },
    );
    await userEvent.click(
      within(screen.getByText("Pricing overrides").parentElement!).getByRole(
        "combobox",
        { name: "Unit" },
      ),
    );
    await userEvent.click(await screen.findByRole("option", { name: "weeks" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Market history" }), {
      target: { value: history },
    });
    await waitFor(
      () => {
        const latest = prices().at(-1);
        expect(latest?.metricPricingOverrides).toBe(
          '{"theta_period":{"count":1,"unit":"weeks"}}',
        );
        expect(latest?.marketHistory).toBe(history);
        expect(latest?.instrumentJson).toContain(
          '"amount":"1000000.123456789"',
        );
      },
      { timeout: budget(5000) },
    );
    await userEvent.click(screen.getByRole("tab", { name: "2 Market" }));
    await userEvent.click(screen.getByRole("tab", { name: "Snapshot JSON" }));
    showOriginal(screen.getByRole("region", { name: "Market snapshot" }));
    const snapshot = screen.getByRole("region", { name: "Market snapshot" });
    expect(snapshot.querySelector("pre")!.textContent).toBe(
      canonicalMarket(fixture.request.marketJson),
    );
    expect(
      snapshot.querySelector("[data-json-print-source]")!.textContent,
    ).toBe(canonicalMarket(fixture.request.marketJson));
    expect(screen.getByRole("region", { name: "Results" })).toBe(results);
    await userEvent.click(screen.getByRole("tab", { name: "1 Instrument" }));
    expect(
      (screen.getByRole("textbox", { name: "Amount" }) as HTMLInputElement)
        .value,
    ).toBe("1000000.123456789");
    expect(mock.createClient).toHaveBeenCalledTimes(1);
    view.unmount();
    expect(close).toHaveBeenCalledTimes(1);
  },
  budget(15000),
);
it(
  "retains the completed context during invalid edits, native pricing failure and catalogue selection",
  async () => {
    mount();
    expect(
      (
        await screen.findByTitle(
          fixture.result.value.amount,
          {},
          { timeout: budget(3000) },
        )
      ).textContent,
    ).toBe(bondDisplay);
    const amount = screen.getByRole("textbox", { name: "Amount" });
    fireEvent.change(amount, { target: { value: "abc" } });
    const issue = await screen.findByRole("button", {
      name: "instrument.spec.notional.amount",
    });
    await userEvent.click(issue);
    expect(document.activeElement).toBe(amount);
    expect(prices()).toHaveLength(1);
    expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
      bondDisplay,
    );
    fireEvent.change(amount, { target: { value: "1000000" } });
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Add theta period" }),
    );
    fireEvent.change(
      within(screen.getByText("Pricing overrides").parentElement!).getByRole(
        "textbox",
        { name: "Count" },
      ),
      {
        target: { value: "0" },
      },
    );
    await userEvent.click(
      within(screen.getByText("Pricing overrides").parentElement!).getByRole(
        "combobox",
        { name: "Unit" },
      ),
    );
    await userEvent.click(await screen.findByRole("option", { name: "weeks" }));
    await waitFor(
      () =>
        expect(
          screen
            .getAllByRole("alert")
            .some((node) => node.textContent?.includes("Invalid tenor")),
        ).toBe(true),
      { timeout: budget(3000) },
    );
    expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
      bondDisplay,
    );
    await userEvent.click(
      screen.getByRole("combobox", { name: "Instrument type" }),
    );
    await userEvent.click(screen.getByRole("option", { name: "equity" }));
    expect(
      await screen.findByRole("button", { name: "Load example" }),
    ).toBeTruthy();
    await waitFor(() => expect(prices()).toHaveLength(3), {
      timeout: budget(3000),
    });
    expect(prices()[2].model).toBe(prices()[0].model);
    expect(screen.getByTitle(fixture.result.value.amount).textContent).toBe(
      bondDisplay,
    );
  },
  budget(15000),
);
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

it(
  "feeds the complete native-validated market edit to pricing and shares stored-node selection",
  async () => {
    mount();
    expect(
      (
        await screen.findByTitle(
          fixture.result.value.amount,
          {},
          { timeout: budget(3000) },
        )
      ).textContent,
    ).toBe(bondDisplay);
    await userEvent.click(screen.getByRole("tab", { name: "2 Market" }));
    await userEvent.click(screen.getByRole("tab", { name: "Edit market" }));
    fireEvent.change(screen.getAllByLabelText("Stored value 1")[0], {
      target: { value: "0.985" },
    });
    await waitFor(() => expect(prices()).toHaveLength(2), {
      timeout: budget(4000),
    });
    const request = prices()[1];
    const expected = JSON.parse(fixture.request.marketJson);
    expected.curves[0].knot_points[1][1] = 0.985;
    expect(request.marketJson).toBe(canonicalMarket(JSON.stringify(expected)));
    expect({ ...request, marketJson: prices()[0].marketJson }).toEqual(
      prices()[0],
    );
    await userEvent.click(screen.getByRole("tab", { name: "View market" }));
    const curveIndex = JSON.parse(request.marketJson).curves.findIndex(
      (curve: any) => curve.id === expected.curves[0].id,
    );
    fireEvent.change(screen.getByLabelText("Search market data"), {
      target: { value: `/curves/${curveIndex}` },
    });
    fireEvent.click(
      screen.getByRole("button", {
        name: `Inspect ${expected.curves[0].id}, /curves/${curveIndex}`,
      }),
    );
    const cell = screen.getByRole("cell", { name: "0.985" });
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
  },
  budget(15000),
);

it(
  "adds the native static-arbitrage badge to a stored surface only when the host supplies forwards",
  async () => {
    const surface = calibrationCases.find((c) =>
      c.source.includes("08_equity"),
    )!.result.result.final_market.surfaces[0]!;
    const market = JSON.parse(fixture.request.marketJson);
    market.surfaces = [surface];
    const surfaceOptions = vi.fn((stored: { id: string }) =>
      stored.id === "AAPL-EQUITY-VOL"
        ? { colorDomain: [0, 1] as const, forwards: [175] }
        : undefined,
    );
    render(
      <FinstackQueryProvider>
        <PricingWorkbench
          defaultRequest={{
            ...fixture.request,
            marketJson: JSON.stringify(market),
          }}
          surfaceOptions={surfaceOptions}
        />
      </FinstackQueryProvider>,
    );
    await userEvent.click(screen.getByRole("tab", { name: "2 Market" }));
    await userEvent.click(screen.getByRole("tab", { name: "View market" }));
    fireEvent.change(
      await screen.findByLabelText(
        "Search market data",
        {},
        { timeout: budget(5000) },
      ),
      { target: { value: "AAPL-EQUITY-VOL" } },
    );
    fireEvent.click(
      screen.getByRole("button", { name: /^Inspect AAPL-EQUITY-VOL/ }),
    );
    const details = await screen.findByRole("button", {
      name: "Arbitrage details",
    });
    await waitFor(
      () => expect(details.parentElement!.textContent).toContain("Passed"),
      { timeout: budget(5000) },
    );
    expect(
      call.mock.calls
        .filter(([method]) => method === "checkSurface")
        .map(([, request]) => request),
    ).toEqual([{ surface, forwards: [175] }]);
    // The stored chart stays beside the badge.
    expect(
      screen.getAllByRole("img", { name: /AAPL-EQUITY-VOL/ }).length,
    ).toBeGreaterThan(0);
  },
  budget(15000),
);

it.each(detailFixtures.cases)(
  "composes actual $type pricing with its returned $detailType route",
  async (entry) => {
    mount(entry.request);
    await userEvent.click(screen.getByRole("tab", { name: "4 Diagnostics" }));
    const raw = await screen.findByRole(
      "region",
      { name: "Complete valuation result JSON", hidden: true },
      { timeout: budget(10000) },
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
  "keeps $type pricing intact with the native cashflow envelope or its actual native export error",
  async (entry) => {
    mount(entry.request);
    const results = within(screen.getByRole("region", { name: "Results" }));
    const tabs = within(results.getByRole("tablist", { name: "Result views" }));
    await userEvent.click(tabs.getByRole("tab", { name: "4 Diagnostics" }));
    const result = await results.findByRole(
      "region",
      { name: "Complete valuation result JSON", hidden: true },
      { timeout: budget(10000) },
    );
    expect(JSON.parse(result.querySelector("pre")!.textContent!).value).toEqual(
      entry.pricedValue,
    );
    await userEvent.click(tabs.getByRole("tab", { name: "3 Cashflows" }));
    if (entry.error) {
      expect(
        await results.findByText(
          "Cashflows are exported only for the discounting and hazard_rate models.",
        ),
      ).toBeTruthy();
      expect(
        call.mock.calls.filter(([method]) => method === "cashflows"),
      ).toHaveLength(0);
    } else {
      const viewer = await results.findByRole("region", {
        name: "Cashflows",
      });
      await within(viewer).findByRole("table", { name: "Cashflow schedule" });
      fireEvent.click(
        within(viewer).getByText("Envelope JSON", { exact: true }),
      );
      await waitFor(() => {
        showOriginal(viewer);
        expect(viewer.querySelector("pre")!.textContent).toBe(
          serializeHost(entry.cashflows),
        );
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
  await waitFor(() => expect(prices()).toHaveLength(1), {
    timeout: budget(5000),
  });
  const request = prices()[0];
  expect(JSON.parse(request.instrumentJson).instrument.type).toBe("equity");
  expect(request.marketJson).toBe(canonicalMarket(fixture.request.marketJson));
  expect(request.model).toBe(fixture.request.model);
});

it(
  "loads the complete host request when a prepared instrument is selected",
  async () => {
    const option = cashflowFixtures.cases.find(
      (entry) => entry.type === "equity_option",
    )!.request;
    render(
      <FinstackQueryProvider>
        <PricingWorkbench
          defaultRequest={fixture.request}
          exampleRequests={{ bond: fixture.request, equity_option: option }}
        />
      </FinstackQueryProvider>,
    );
    await waitFor(() => expect(prices()).toHaveLength(1), {
      timeout: budget(5000),
    });
    expect(screen.queryByRole("button", { name: "Load example" })).toBeNull();
    await userEvent.click(
      screen.getByRole("combobox", { name: "Instrument type" }),
    );
    expect(screen.queryByRole("option", { name: "equity" })).toBeNull();
    await userEvent.click(
      screen.getByRole("option", { name: "equity_option" }),
    );
    await waitFor(() => expect(prices()).toHaveLength(2), {
      timeout: budget(5000),
    });
    const selected = prices()[1];
    expect(JSON.parse(selected.instrumentJson).instrument.type).toBe(
      "equity_option",
    );
    expect(selected.marketJson).toBe(canonicalMarket(option.marketJson));
    expect(selected.asOf).toBe(option.asOf);
    expect(selected.model).toBe(option.model);
    expect(selected.metrics).toEqual(option.metrics);
    expect(
      (
        await screen.findByTitle(
          cashflowFixtures.cases.find(
            (entry) => entry.type === "equity_option",
          )!.pricedValue.amount,
        )
      ).textContent,
    ).toContain("25,748");
  },
  budget(15000),
);

it("offers scenarios only while structured credit is selected", async () => {
  render(
    <FinstackQueryProvider>
      <PricingWorkbench
        defaultRequest={fixture.request}
        scenario={{
          trancheId: scenarioCases.trancheId,
          gridJson: JSON.stringify(scenarioCases.grid),
          priceDomain: [80, 140],
        }}
      />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(prices()).toHaveLength(1), {
    timeout: budget(10000),
  });
  expect(screen.queryByRole("tab", { name: "Scenarios" })).toBeNull();
});
it(
  "loads scenarios only on demand from the completed structured-credit context",
  async () => {
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
    await waitFor(() => expect(prices()).toHaveLength(1), {
      timeout: budget(10000),
    });
    expect(call.mock.calls.some(([method]) => method === "scenarioTable")).toBe(
      false,
    );
    await userEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
    const toggle = await screen.findByText(
      "Scenarios",
      { selector: "summary" },
      { timeout: budget(5000) },
    );
    fireEvent.click(toggle);
    await waitFor(
      () =>
        expect(
          call.mock.calls.some(([method]) => method === "scenarioTable"),
        ).toBe(true),
      { timeout: budget(5000) },
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
          document.querySelector('[aria-label="CLONOTES-A scenario measures"]'),
        ).toBeTruthy(),
      { timeout: budget(5000) },
    );
    expect(prices()).toHaveLength(1);
    // The ScenarioGrid editor is seeded from the host grid; an edit runs only on submit.
    // Queries stay inside the disclosure: the whole workbench DOM is large.
    const panel = within(toggle.closest("details")!);
    const cpr = panel.getByLabelText(/^cprs 2$/i) as HTMLInputElement;
    expect(Number(cpr.value)).toBe(scenarioCases.grid.cprs[1]);
    fireEvent.change(cpr, { target: { value: "0.1" } });
    const run = panel.getByRole("button", {
      name: "Run scenarios",
    }) as HTMLButtonElement;
    await waitFor(() => expect(run.disabled).toBe(false), {
      timeout: budget(5000),
    });
    const tables = () =>
      call.mock.calls.filter(([method]) => method === "scenarioTable");
    expect(tables()).toHaveLength(1);
    fireEvent.click(run);
    await waitFor(() => expect(tables()).toHaveLength(2), {
      timeout: budget(5000),
    });
    const edited = tables()[1]![1];
    expect({ ...edited, gridJson: undefined }).toEqual({
      ...invocation,
      gridJson: undefined,
    });
    expect(JSON.parse(edited.gridJson)).toEqual({
      ...scenarioCases.grid,
      cprs: [scenarioCases.grid.cprs[0], 0.1, scenarioCases.grid.cprs[2]],
    });
    await waitFor(
      () =>
        expect(
          document.querySelector('[aria-label="CLONOTES-A scenario measures"]'),
        ).toBeTruthy(),
      { timeout: budget(5000) },
    );
  },
  budget(60000),
);

it(
  "enters a deal from a registry family and analyses its tranche with no host seeds",
  async () => {
    const family = standardDeals.cases.find((c) => c.family === "rmbs")!;
    render(
      <FinstackQueryProvider>
        <PricingWorkbench
          defaultRequest={{
            instrumentJson: JSON.stringify(scenarioCases.cases[0]!.instrument),
            marketJson: JSON.stringify(pricingMarket),
            asOf: scenarioCases.asOf,
            model: "discounting",
            metrics: [],
          }}
        />
      </FinstackQueryProvider>,
    );
    await waitFor(() => expect(prices()).toHaveLength(1), {
      timeout: budget(10000),
    });
    // The form starts from Rust's example, and building sends that family and those params.
    const summary = screen.getByText("Start from a standard deal", {
      selector: "summary",
    });
    const details = summary.closest("details")!;
    details.open = true;
    fireEvent(details, new Event("toggle"));
    const entry = within(details);
    fireEvent.click(await entry.findByRole("radio", { name: "RMBS" }));
    const build = entry.getByRole("button", {
      name: "Build deal",
    }) as HTMLButtonElement;
    await waitFor(() => expect(build.disabled).toBe(false), {
      timeout: budget(5000),
    });
    fireEvent.click(build);
    await waitFor(
      () =>
        expect(
          call.mock.calls.some(([method]) => method === "newStandardDeal"),
        ).toBe(true),
      { timeout: budget(5000) },
    );
    const built = call.mock.calls.find(
      ([method]) => method === "newStandardDeal",
    )![1];
    expect(built.dealType).toBe("rmbs");
    expect(JSON.parse(built.paramsJson)).toEqual(standardDeals.params);
    // The built deal reprices through the workbench's normal path.
    await waitFor(
      () => expect(prices().at(-1)!.instrumentJson).toBe(family.instrumentJson),
      { timeout: budget(10000) },
    );
    // No host seed: the tab lists the deal's native tranches and selects the first returned.
    await userEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
    const tranche = family.tranches.tranches[0]!.id;
    await waitFor(
      () =>
        expect(
          document.querySelector(`[aria-label="${tranche} tranche analytics"]`),
        ).toBeTruthy(),
      { timeout: budget(10000) },
    );
    const metricCalls = call.mock.calls.filter(
      ([method]) => method === "trancheMetrics",
    );
    expect(metricCalls.at(-1)![1]).toMatchObject({
      instrumentJson: family.instrumentJson,
      trancheId: tranche,
    });
    // The built deal reaches the pool report, which runs only when opened.
    expect(call.mock.calls.some(([method]) => method === "poolReport")).toBe(
      false,
    );
    fireEvent.click(screen.getByText("Pool report", { selector: "summary" }));
    await waitFor(
      () =>
        expect(
          document.querySelector('[aria-label="Pool report"] table'),
        ).toBeTruthy(),
      { timeout: budget(10000) },
    );
    expect(
      call.mock.calls.find(([method]) => method === "poolReport")![1],
    ).toEqual({
      instrumentJson: family.instrumentJson,
      asOf: expect.any(String),
    });
    // ... and the waterfall report, on the completed request's market and date.
    expect(call.mock.calls.some(([method]) => method === "runSimulation")).toBe(
      false,
    );
    fireEvent.click(
      screen.getByText("Waterfall report", { selector: "summary" }),
    );
    await waitFor(
      () =>
        expect(
          document.querySelector('[aria-label="Waterfall report"] table'),
        ).toBeTruthy(),
      { timeout: budget(10000) },
    );
    const simulated = call.mock.calls.find(
      ([method]) => method === "runSimulation",
    )![1];
    const lastPrice = prices().at(-1)!;
    expect(simulated).toEqual({
      instrumentJson: family.instrumentJson,
      marketJson: lastPrice.marketJson,
      asOf: lastPrice.asOf,
    });
    // ... and the coverage tests, from that same simulation in Rust's units.
    fireEvent.click(
      screen.getByText("Coverage tests", { selector: "summary" }),
    );
    await waitFor(
      () =>
        expect(
          document.querySelector('[aria-label="Coverage tests"]') ??
            screen.queryByText(/returned no coverage tests/),
        ).toBeTruthy(),
      { timeout: budget(10000) },
    );
    expect(
      call.mock.calls.filter(([method]) => method === "runSimulation"),
    ).toHaveLength(1);
    // ... and the enhancement chart, from that same simulation.
    fireEvent.click(
      screen.getByText("Credit enhancement and excess spread", {
        selector: "summary",
      }),
    );
    await waitFor(
      () =>
        expect(
          document.querySelector(
            '[aria-label="Credit enhancement and excess spread"]',
          ),
        ).toBeTruthy(),
      { timeout: budget(10000) },
    );
    expect(
      call.mock.calls.filter(([method]) => method === "runSimulation"),
    ).toHaveLength(1);
    // ... and the equity tranche: the call is always made and Rust answers for the deal.
    fireEvent.click(
      screen.getByText("Equity tranche", { selector: "summary" }),
    );
    await waitFor(
      () =>
        expect(
          screen
            .getByRole("region", { name: "Equity tranche" })
            .querySelector('[role="alert"]')?.textContent,
        ).toMatch(/deal has no equity tranche/),
      { timeout: budget(10000) },
    );
    const equity = call.mock.calls.find(
      ([method]) => method === "equityMetrics",
    )![1];
    // A blank price is not a price: JS leaves the default to Rust.
    expect(equity.purchasePricePct).toBeNull();
    expect(equity.instrumentJson).toBe(family.instrumentJson);
    // The grid and the price extent come from Rust's example and the returned prices.
    fireEvent.click(screen.getByText("Scenarios", { selector: "summary" }));
    await waitFor(
      () =>
        expect(
          document.querySelector(`[aria-label="${tranche} scenario measures"]`),
        ).toBeTruthy(),
      { timeout: budget(10000) },
    );
    expect(
      JSON.parse(
        call.mock.calls.find(([method]) => method === "scenarioTable")![1]
          .gridJson,
      ),
    ).toEqual(scenarioGridSchema.examples[0]);
  },
  budget(60000),
);

it(
  "prepares one completed cashflow snapshot before printing and restores the input tab",
  async () => {
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
      const button = await screen.findByRole("button", {
        name: "Print report",
      });
      await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false), {
        timeout: budget(10000),
      });
      const request = prices()[0]!;
      fireEvent.click(button);
      const workbench = screen.getByRole("region", {
        name: "Pricing workbench",
      });
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
        model: "discounting",
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
  },
  budget(30000),
);

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
it(
  "prices the applied market scenario beside the base valuation and never subtracts them",
  async () => {
    const roll = marketScenarios.cases.find(
      (entry) => entry.name === "roll-1m-25bp",
    )!;
    render(
      <FinstackQueryProvider>
        <PricingWorkbench
          defaultRequest={fixture.request}
          marketScenario={roll.scenarioJson}
        />
      </FinstackQueryProvider>,
    );
    const results = screen.getByRole("region", { name: "Results" });
    await within(results).findByTitle(
      fixture.result.value.amount,
      {},
      { timeout: budget(3000) },
    );
    expect(within(results).queryByText("Comparison valuation")).toBeNull();
    await userEvent.click(
      within(screen.getByRole("tablist", { name: "Pricing inputs" })).getByRole(
        "tab",
        { name: "Market scenario" },
      ),
    );
    const apply = screen.getByRole("button", { name: "Apply scenario" });
    await waitFor(
      () => expect((apply as HTMLButtonElement).disabled).toBe(false),
      {
        timeout: budget(3000),
      },
    );
    expect(
      call.mock.calls.filter(([method]) => method === "applyMarketScenario"),
    ).toEqual([]);
    await userEvent.click(apply);
    await waitFor(() => expect(prices()).toHaveLength(2), {
      timeout: budget(5000),
    });
    const [base, shocked] = prices();
    expect(
      call.mock.calls.filter(([method]) => method === "applyMarketScenario"),
    ).toEqual([
      [
        "applyMarketScenario",
        {
          scenarioJson: roll.scenarioJson,
          marketJson: base!.marketJson,
          asOf: base!.asOf,
          instrumentJson: base!.instrumentJson,
        },
      ],
    ]);
    // Same model, metrics and instrument; the rolled date and the shocked market.
    expect(shocked).toEqual({
      ...base,
      asOf: "2025-02-03",
      marketJson: shocked!.marketJson,
    });
    expect(shocked!.marketJson).not.toBe(base!.marketJson);
    await within(results).findByTitle(
      roll.shocked.value.amount,
      {},
      { timeout: budget(3000) },
    );
    expect(within(results).getByText("Comparison valuation")).toBeTruthy();
    expect(
      within(results).getByTitle(fixture.result.value.amount),
    ).toBeTruthy();
    expect(within(results).getByText("2025-02-03")).toBeTruthy();
    await userEvent.click(
      within(screen.getByRole("tablist", { name: "Result views" })).getByRole(
        "tab",
        { name: "Market scenario" },
      ),
    );
    const timeRoll = await within(results).findByRole("region", {
      name: "Time roll",
    });
    expect(within(timeRoll).getAllByTitle("4613.745264131")).toHaveLength(2);
    // Rust's horizon return of the same base request under the applied
    // scenario; method and calendar stay unset.
    expect(
      call.mock.calls.filter(([method]) => method === "horizonReturn"),
    ).toEqual([
      [
        "horizonReturn",
        {
          instrumentJson: base!.instrumentJson,
          marketJson: base!.marketJson,
          asOf: base!.asOf,
          scenarioJson: roll.scenarioJson,
          method: null,
          calendarId: null,
        },
      ],
    ]);
    const horizon = within(results).getByRole("region", {
      name: "Horizon return",
    });
    const expected = native.computeHorizonReturn(
      base!.instrumentJson,
      base!.marketJson,
      base!.asOf,
      roll.scenarioJson,
    );
    await within(horizon).findByText(
      String(expected.summary.annualized_return),
      {},
      { timeout: budget(5000) },
    );
    expect(within(horizon).getByText(/Horizon: 33 days/)).toBeTruthy();
    await userEvent.click(
      screen.getByRole("button", { name: "Clear scenario" }),
    );
    await waitFor(() =>
      expect(within(results).queryByText("Comparison valuation")).toBeNull(),
    );
    expect(prices()).toHaveLength(2);
  },
  budget(20000),
);

it(
  "explains the P&L from a pinned T0 market to an edited, repriced market with Rust's attribution only",
  async () => {
    mount();
    const results = screen.getByRole("region", { name: "Results" });
    await within(results).findByTitle(
      fixture.result.value.amount,
      {},
      { timeout: budget(3000) },
    );
    await userEvent.click(
      within(screen.getByRole("tablist", { name: "Result views" })).getByRole(
        "tab",
        { name: "P&L explain" },
      ),
    );
    await userEvent.click(screen.getByRole("button", { name: "Pin as T0" }));
    expect(within(results).getByText(/T0 pinned at/).textContent).toBe(
      `T0 pinned at ${fixture.request.asOf}`,
    );
    // Rust requires a method; none is preselected and nothing is requested.
    expect(
      within(results).getByText("Choose an attribution method."),
    ).toBeTruthy();
    await userEvent.click(screen.getByRole("tab", { name: "2 Market" }));
    await userEvent.click(screen.getByRole("tab", { name: "Edit market" }));
    fireEvent.change(screen.getAllByLabelText("Stored value 1")[0], {
      target: { value: "0.985" },
    });
    await waitFor(() => expect(prices()).toHaveLength(2), {
      timeout: budget(4000),
    });
    const [t0, t1] = prices();
    await userEvent.click(
      within(results).getByRole("radio", { name: "parallel" }),
    );
    await waitFor(
      () =>
        expect(
          call.mock.calls.filter(([method]) => method === "attributePnlMany"),
        ).toHaveLength(1),
      { timeout: budget(4000) },
    );
    // The pinned market and date, and the last priced instrument, market and date.
    expect(
      call.mock.calls.filter(([method]) => method === "attributePnlMany"),
    ).toEqual([
      [
        "attributePnlMany",
        {
          instrumentJsons: [t1!.instrumentJson],
          marketT0Json: t0!.marketJson,
          marketT1Json: t1!.marketJson,
          asOfT0: t0!.asOf,
          asOfT1: t1!.asOf,
          method: "parallel",
        },
      ],
    ]);
    const [expected] = native.attributePnlMany(
      `{"as_of_t0":"${t0!.asOf}","as_of_t1":"${t1!.asOf}","market_t0":${t0!.marketJson},"market_t1":${t1!.marketJson},"method":"parallel"}`,
      [t1!.instrumentJson],
    );
    const report = within(results).getByRole("region", {
      name: "P&L attribution",
    });
    expect(
      await within(report).findAllByTitle(
        expected.total_pnl.amount,
        {},
        { timeout: budget(4000) },
      ),
    ).not.toHaveLength(0);
    expect(
      within(report).getAllByTitle(expected.rates_curves_pnl.amount),
    ).not.toHaveLength(0);
    // The scenario source stays unavailable until a scenario is applied.
    expect(
      within(results)
        .getByRole("radio", { name: "Applied market scenario" })
        .getAttribute("aria-disabled"),
    ).toBe("true");
  },
  budget(20000),
);
