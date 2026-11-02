// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  BondCreditCard,
  bondCreditMetrics,
} from "@/components/finstack/valuations/components/bond-credit-card/bond-credit-card";
import {
  applicableMetrics,
  type MetricValueRow,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { unwrap } from "@/workers/finstack-contract";
import type { MetricMetadata } from "finstack-quant-wasm";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
afterEach(cleanup);
const { cases, available } = fixture;
const metadata = fixture.metadata as unknown as MetricMetadata[];
const rowsOf = (entry: { rows: unknown }) =>
  entry.rows as unknown as MetricValueRow[];
const failing = (entry: { rows: unknown }) =>
  rowsOf(entry).flatMap((row) => (row.value.ok ? [] : [row.metric]));
const value = (entry: { rows: unknown }, metric: string) =>
  (
    rowsOf(entry).find((row) => row.metric === metric)!.value as {
      value: number;
    }
  ).value;
const rowFor = (metric: string) =>
  screen.getByRole("rowheader", { name: labelFor(metric) }).closest("tr")!;

// Values are regenerated and compared by `mise run ui-fixtures --check`.
it("pins the JSON inputs the fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/valuations/bond-credit/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it("Rust lists every id of the card for a bond", () => {
  expect(cases.bullet.request.metrics).toEqual([...bondCreditMetrics]);
  expect(applicableMetrics(bondCreditMetrics, available)).toEqual([
    ...bondCreditMetrics,
  ]);
});
it.each(Object.entries(cases))(
  "the worker returns the native values and errors of %s, overrides applied",
  async (_name, entry) => {
    const rows = unwrap(await harness.proxy.metricValues(entry.request));
    expect(rows).toEqual(entry.rows);
  },
  budget(30000),
);
it("a bond with no government curve, no hazard curve and no config errors in those rows alone", () => {
  const entry = cases.bullet;
  expect(failing(entry)).toEqual([
    "g_spread",
    "oas",
    "discount_margin",
    "recovery01",
    "jump_to_default",
    "breakeven",
  ]);
  render(<BondCreditCard rows={rowsOf(entry)} metadata={metadata} />);
  const reason = (metric: string) =>
    within(rowFor(metric)).getByRole("alert").textContent;
  expect(reason("g_spread")).toContain("requires a government curve");
  expect(reason("recovery01")).toContain("requires hazard pricing");
  expect(reason("jump_to_default")).toContain("requires hazard pricing");
  expect(reason("breakeven")).toContain("set BreakevenConfig");
  expect(screen.getAllByRole("alert")).toHaveLength(failing(entry).length);
  for (const metric of ["z_spread", "duration_mod", "cs01", "carry_total"])
    expect(rowFor(metric).children[1].textContent).toBe(
      String(value(entry, metric)),
    );
});
it("g_spread prices over a named curve and differs from i_spread over the discount curve", () => {
  const entry = cases.quoted;
  expect(failing(entry)).toEqual([
    "discount_margin",
    "recovery01",
    "jump_to_default",
  ]);
  expect(value(entry, "g_spread")).not.toBe(value(entry, "i_spread"));
});
it("the breakeven of a linear z-spread target is carry over CS01 in basis points, over the horizon", () => {
  const entry = cases.quoted;
  expect(value(entry, "theta_period_days")).toBe(92);
  expect(value(entry, "breakeven")).toBeCloseTo(
    -value(entry, "carry_total") / value(entry, "cs01"),
    9,
  );
  expect(value(entry, "breakeven")).toBeCloseTo(13.861172, 5);
});
it("the breakeven of a linear yield target divides by DV01", () => {
  const entry = cases.callable;
  expect(value(entry, "breakeven")).toBeCloseTo(
    -value(entry, "carry_total") / value(entry, "dv01"),
    9,
  );
});
it("shows the option value and OAS of a callable under the tree model", () => {
  const entry = cases.callable;
  expect(failing(entry)).toEqual([
    "g_spread",
    "discount_margin",
    "recovery01",
    "jump_to_default",
  ]);
  expect(value(entry, "embedded_option_value")).toBeLessThan(0);
  render(<BondCreditCard rows={rowsOf(entry)} metadata={metadata} />);
  expect(rowFor("oas").children[1].textContent).toBe(
    String(value(entry, "oas")),
  );
});
it("prices default risk on a hazard curve, and a curve with no recipe errors CS01 and what needs it", () => {
  const entry = cases.hazard;
  expect(failing(entry)).toEqual([
    "g_spread",
    "oas",
    "discount_margin",
    "cs01",
    "breakeven",
  ]);
  render(<BondCreditCard rows={rowsOf(entry)} metadata={metadata} />);
  expect(rowFor("recovery01").children[1].textContent).toBe(
    String(value(entry, "recovery01")),
  );
  expect(rowFor("jump_to_default").children[1].textContent).toBe(
    String(value(entry, "jump_to_default")),
  );
  expect(within(rowFor("breakeven")).getByRole("alert").textContent).toContain(
    "calibration recipe",
  );
});
it("a floating coupon has a discount margin and no par asset swap", () => {
  expect(failing(cases.frn)).toEqual([
    "g_spread",
    "asw_par",
    "recovery01",
    "jump_to_default",
  ]);
});
it("states unit and group as Rust declares them, and no unit for breakeven", () => {
  render(<BondCreditCard rows={rowsOf(cases.quoted)} metadata={metadata} />);
  const cells = (metric: string) =>
    [...rowFor(metric).children].map((cell) => cell.textContent).slice(2);
  expect(cells("z_spread")).toEqual(["decimal", "Pricing"]);
  expect(cells("dv01")).toEqual(["currency", "Sensitivity"]);
  expect(cells("recovery01")).toEqual(["currency", "Credit"]);
  expect(cells("carry_total")).toEqual(["currency", "Carry"]);
  expect(cells("breakeven")).toEqual(["not declared", "Carry"]);
});
it("shows the whole-request error and the loading state", () => {
  const { rerender } = render(
    <BondCreditCard rows={null} error="bad market" />,
  );
  expect(screen.getByRole("alert").textContent).toBe("bad market");
  rerender(<BondCreditCard rows={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Pricing metrics…");
});
it("hides the form until a host takes the applied overrides", () => {
  render(<BondCreditCard rows={rowsOf(cases.bullet)} metadata={metadata} />);
  expect(screen.queryByText("Carry horizon and breakeven")).toBeNull();
});
it(
  "applies the schema form's overrides, keeping the fields it does not edit",
  async () => {
    const onOverridesChange = vi.fn();
    const supplied = {
      bump_config: { rate_bump_bp: 2 },
      theta_period: { count: 3, unit: "months" },
      breakeven_config: { target: "ytm", mode: "linear" },
    };
    render(
      <BondCreditCard
        rows={rowsOf(cases.bullet)}
        metadata={metadata}
        overrides={JSON.stringify(supplied)}
        onOverridesChange={onOverridesChange}
      />,
    );
    await userEvent.click(screen.getByText("Carry horizon and breakeven"));
    const apply = screen.getByRole("button", {
      name: "Apply",
    }) as HTMLButtonElement;
    // Apply waits for the (structural) validation to settle.
    await waitFor(() => expect(apply.disabled).toBe(false), {
      timeout: budget(5000),
    });
    await userEvent.click(apply);
    await waitFor(() => expect(onOverridesChange).toHaveBeenCalledTimes(1), {
      timeout: budget(5000),
    });
    expect(JSON.parse(onOverridesChange.mock.calls[0][0])).toEqual(
      expect.objectContaining(supplied),
    );
    // The carry horizon and target offered are the schema's; nothing else shows.
    expect(screen.queryByText("Bump config")).toBeNull();
  },
  budget(15000),
);
it(
  "sends what the form applied to the worker, which moves carry and breakeven",
  async () => {
    // The bullet's fixture has no overrides: one day, and no breakeven.
    const applied = cases.hazard.request.metricPricingOverrides;
    const rows = unwrap(
      await harness.proxy.metricValues({
        ...cases.bullet.request,
        metricPricingOverrides: applied,
      }),
    ) as unknown as MetricValueRow[];
    const days = rows.find((row) => row.metric === "theta_period_days")!;
    expect(days.value).toEqual({ ok: true, value: 92 });
    expect(value(cases.bullet, "theta_period_days")).toBe(1);
    expect(rows.find((row) => row.metric === "breakeven")!.value.ok).toBe(true);
  },
  budget(30000),
);
