// @vitest-environment jsdom
import { expect, it, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import cashflowFixtures from "./cashflows/cases.json";
import { budget } from "../shared/budget";
import calibrationCases from "../calibration/cases.json";
import marketScenarios from "../scenarios/application/cases.json";
import { createClient } from "../shared/worker/client";
import {
  native,
  call,
  close,
  canonicalMarket,
  showOriginal,
  prices,
  mount,
  fixture,
  bondDisplay,
} from "./workbench-harness";
// The one-row annual cashflow ladder repeats the bond's exact PV, so the
// headline valuation is the Money value carrying that title.
function headline(scope: Pick<typeof screen, "getAllByTitle"> = screen) {
  const nodes = scope
    .getAllByTitle(fixture.result.value.amount)
    .filter((node) => node.classList.contains("finstack-money"));
  expect(nodes).toHaveLength(1);
  return nodes[0];
}
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
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
    expect(createClient).toHaveBeenCalledTimes(1);
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
    expect(headline().textContent).toBe(bondDisplay);
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
    expect(headline().textContent).toBe(bondDisplay);
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
    expect(headline().textContent).toBe(bondDisplay);
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
    ).toEqual([{ surface, config: { forward_prices: [175] } }]);
    // The stored chart stays beside the badge.
    expect(
      screen.getAllByRole("img", { name: /AAPL-EQUITY-VOL/ }).length,
    ).toBeGreaterThan(0);
  },
  budget(15000),
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
    expect(headline(within(results))).toBeTruthy();
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
    expect(within(timeRoll).getAllByTitle("4613.7452641314")).toHaveLength(2);
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
          inputs: {
            as_of_t0: t0!.asOf,
            as_of_t1: t1!.asOf,
            method: "parallel",
          },
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
