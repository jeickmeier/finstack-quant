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
import fixture from "../src/fixtures/results/bond.json";
import {
  type PriceRequest,
  FinstackError,
  errorValue,
} from "../registry/workers/finstack-contract";
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
      case "validate":
        return {
          json: native.validateInstrumentJson(request.instrumentJson),
          revision: request.revision,
        };
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
const prices = () =>
  call.mock.calls
    .filter(([method]) => method === "price")
    .map(([, request]) => request as PriceRequest);
function mount(request?: PriceRequest) {
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
    instrumentJson: native.validateInstrumentJson(
      fixture.request.instrumentJson,
    ),
  });
  const results = screen.getByRole("region", { name: "Results" });
  await within(results).findByText("USD 1,042,500");
  const amount = screen.getByRole("textbox", { name: "Amount" });
  fireEvent.change(amount, { target: { value: "1000000.123456789" } });
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
  expect(
    within(screen.getByRole("region", { name: "Market snapshot" })).getByText(
      fixture.request.marketJson,
    ),
  ).toBeTruthy();
  expect(screen.getByRole("region", { name: "Results" })).toBe(results);
  await userEvent.click(screen.getByRole("tab", { name: "1 Instrument" }));
  expect(
    (screen.getByRole("textbox", { name: "Amount" }) as HTMLInputElement).value,
  ).toBe("1000000.123456789");
  expect(mock.createClient).toHaveBeenCalledTimes(1);
  view.unmount();
  expect(close).toHaveBeenCalledTimes(1);
});
it("retains the completed context during invalid edits, native pricing failure and unsupported selection", async () => {
  mount();
  await screen.findByText("USD 1,042,500", {}, { timeout: 3000 });
  const amount = screen.getByRole("textbox", { name: "Amount" });
  fireEvent.change(amount, { target: { value: "abc" } });
  const issue = await screen.findByRole("button", {
    name: "instrument.spec.notional.amount",
  });
  await userEvent.click(issue);
  expect(document.activeElement).toBe(amount);
  expect(prices()).toHaveLength(1);
  expect(screen.getByText("USD 1,042,500")).toBeTruthy();
  fireEvent.change(amount, { target: { value: "1000000" } });
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
  expect(screen.getByText("USD 1,042,500")).toBeTruthy();
  await userEvent.click(
    screen.getByRole("combobox", { name: "Instrument type" }),
  );
  await userEvent.click(screen.getByRole("option", { name: "equity" }));
  expect(
    await screen.findByRole("button", { name: "Load example" }),
  ).toBeTruthy();
  expect(screen.getByText("Instrument not yet supported")).toBeTruthy();
  expect(prices()).toHaveLength(2);
});
it("rejects a calibration envelope as a market snapshot before issuing a price request", async () => {
  mount({
    ...fixture.request,
    marketJson: '{"schema_version":1,"calibration":{"type":"discount"}}',
  });
  const marketTab = screen.getByRole("tab", { name: "2 Market" });
  await userEvent.click(marketTab);
  expect(
    within(screen.getByRole("region", { name: "Market snapshot" })).getByRole(
      "alert",
    ),
  ).toBeTruthy();
  await waitFor(() =>
    expect(call.mock.calls.some(([method]) => method === "validate")).toBe(
      true,
    ),
  );
  expect(prices()).toEqual([]);
});
