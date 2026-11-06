// @vitest-environment jsdom
import { expect, it, vi } from "vitest";
import { screen, fireEvent, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { serializeHost } from "../../src/codec.mjs";
import cashflowFixtures from "./cashflows/cases.json";
import { budget } from "../shared/budget";
import { native, call, showOriginal, prices, mount } from "./workbench-harness";
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
