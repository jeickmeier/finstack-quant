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
import scenarioCases from "./scenario-table/cases.json";
import standardDeals from "./scenario-table/standard-deals.json";
import scenarioGridSchema from "../../src/generated/schemas/scenario_grid.json";
import pricingMarket from "./instruments/pricing-market.json";
import { budget } from "../shared/budget";
import { native, call, prices } from "./workbench-harness";
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
    fireEvent.click(
      await entry.findByRole(
        "radio",
        { name: "RMBS" },
        { timeout: budget(10000) },
      ),
    );
    expect(entry.getByText(family.profileId)).toBeTruthy();
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
    expect(JSON.parse(built.paramsJson)).toEqual(family.params);
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
      asOf: scenarioCases.asOf,
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
        // The family's registry deal configures no coverage test.
        expect(
          screen.queryByText("No coverage test was evaluated."),
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
