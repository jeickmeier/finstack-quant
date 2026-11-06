// Shared setup of the pricing workbench suites (workbench*.test.tsx): the
// mocked worker client answering through the Node WASM facade, and helpers.
import { beforeEach, afterEach, vi } from "vitest";
import { render, fireEvent, cleanup, within } from "@testing-library/react";
import { createRequire } from "node:module";
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import type { ValuationResult } from "finstack-quant-wasm";
import bondFixture from "../../src/fixtures/results/bond.json";
import {
  type PriceRequest,
  FinstackError,
  errorValue,
  unwrap,
} from "@/workers/finstack-contract";
import { formatMoney, formatRawMoney } from "../../src/format/format";
import { createContext } from "@/workers/services/context";
import { createScenariosService } from "@/workers/services/scenarios";
import { createAttributionService } from "@/workers/services/attribution";
import { createModelsService } from "@/workers/services/models";
import { createClient } from "../shared/worker/client";
import { priceExtras } from "../shared/price-extras.mjs";
/** The JSON import widens currency codes to `string`; the fixture is a native result. */
export const fixture = bondFixture as typeof bondFixture & {
  result: ValuationResult;
};
export const bondDisplay = formatRawMoney(fixture.result.value);
export const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
export let call: ReturnType<typeof vi.fn>, close: ReturnType<typeof vi.fn>;
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
      case "standardDealFamilies":
        return native.standardDealFamilies();
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
      case "priceDriverFields":
        return native.priceDriverFields();
      case "cashflowLadder":
        return native.calendarYearLadderByCurrency(request);
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
            priceExtras(request),
          );
        } catch (error) {
          throw new FinstackError(errorValue(error));
        }
    }
  });
  close = vi.fn();
  createClient.mockReset().mockReturnValue({ call, close });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
export const canonicalMarket = (json: string) => {
  const market = native.MarketContext.fromJson(json);
  try {
    return market.toJson();
  } finally {
    market.free();
  }
};
export function showOriginal(viewer: HTMLElement) {
  const button = within(viewer).queryByRole("button", {
    name: "Original",
    hidden: true,
  });
  if (button) fireEvent.click(button);
}
export const prices = () =>
  call.mock.calls
    .filter(([method]) => method === "price")
    .map(([, request]) => request as PriceRequest);
export function mount(request: PriceRequest = fixture.request) {
  return render(
    <div>
      <h1>Host route</h1>
      <FinstackQueryProvider>
        <PricingWorkbench defaultRequest={request} />
      </FinstackQueryProvider>
    </div>,
  );
}
