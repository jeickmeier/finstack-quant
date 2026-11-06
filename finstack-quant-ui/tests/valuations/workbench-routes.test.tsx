// @vitest-environment jsdom
import { expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { serializeHost } from "../../src/codec.mjs";
import detailFixtures from "./details/cases.json";
import { budget } from "../shared/budget";
import { call, showOriginal, prices, mount } from "./workbench-harness";
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
