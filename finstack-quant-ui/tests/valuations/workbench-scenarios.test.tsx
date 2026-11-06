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
import pricingMarket from "./instruments/pricing-market.json";
import { budget } from "../shared/budget";
import { call, prices, fixture } from "./workbench-harness";
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
  "passes a seeded tranche Rust does not know through, so Rust's error shows",
  async () => {
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
          scenario={{
            trancheId: "NO-SUCH-TRANCHE",
            gridJson: JSON.stringify(scenarioCases.grid),
          }}
        />
      </FinstackQueryProvider>,
    );
    await waitFor(() => expect(prices()).toHaveLength(1), {
      timeout: budget(10000),
    });
    await userEvent.click(screen.getByRole("tab", { name: "Scenarios" }));
    await waitFor(
      () =>
        expect(
          screen
            .getAllByRole("alert")
            .some((alert) => /NO-SUCH-TRANCHE/.test(alert.textContent!)),
        ).toBe(true),
      { timeout: budget(10000) },
    );
    expect(
      call.mock.calls.find(([method]) => method === "trancheMetrics")![1]
        .trancheId,
    ).toBe("NO-SUCH-TRANCHE");
  },
  budget(15000),
);

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
