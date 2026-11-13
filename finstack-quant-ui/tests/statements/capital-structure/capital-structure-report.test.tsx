// @vitest-environment jsdom
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { hashKey } from "@tanstack/react-query";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  capitalStructureSummaryOptions,
  useCapitalStructureSummary,
} from "@/hooks/statements/use-capital-structure-summary/use-capital-structure-summary";
import {
  CapitalStructureReport,
  maturityBars,
} from "@/components/finstack/statements/components/capital-structure-report/capital-structure-report";
import {
  unwrap,
  type CapitalStructureSummary,
} from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { groupDecimal } from "@/lib/finstack/format/format";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";

// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness(["capitalStructureSummary"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
const resultJson = JSON.stringify(fixture.result);
const modelJson = JSON.stringify(fixture.model);
const nodeIds = Object.keys(fixture.model.nodes);
const periodIds = fixture.model.periods.periods.map((period) => period.id);
const summaryOf = (name: keyof typeof fixture.cases) =>
  fixture.cases[name].summary as unknown as CapitalStructureSummary;
const full = summaryOf("full");
const props = { nodeIds, periodIds, onSubmit: () => {} };
/** The item's default display: ratios to four decimals, shares as percents to two. */
const ratio = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
const share = new Intl.NumberFormat("en-US", {
  style: "percent",
  maximumFractionDigits: 2,
}).format;
const money = (value: { amount: string }) => groupDecimal(value.amount);
it("returns the fixture's summaries and rejections through the worker", async () => {
  // netCash is a summary of another model's result.
  for (const [name, { config, summary }] of Object.entries(fixture.cases)) {
    if (name === "netCash") continue;
    expect(
      unwrap(
        await harness.proxy.capitalStructureSummary({
          resultJson,
          modelJson,
          configJson: JSON.stringify(config),
        }),
      ),
    ).toEqual(summary);
  }
  const rejected = await harness.proxy.capitalStructureSummary({
    resultJson,
    modelJson,
    configJson: JSON.stringify(fixture.errors.missingNode.config),
  });
  expect(rejected.ok).toBe(false);
});

it("shows the headline figures and the LTM periods Rust found", () => {
  render(<CapitalStructureReport {...props} summary={full} />);
  const value = (label: string) =>
    screen.getByText(label, { selector: "dt" }).nextElementSibling!.textContent;
  expect(value("Total debt")).toBe(money(full.total_debt));
  expect(value("Cash")).toBe(money(full.cash!));
  expect(value("Net debt")).toBe(money(full.net_debt!));
  expect(value("LTM EBITDA")).toBe(money(full.ltm_ebitda!));
  expect(value("Total leverage")).toBe(ratio(full.total_leverage!));
  expect(value("Net leverage")).toBe(ratio(full.net_leverage!));
  expect(value("Total coverage")).toBe(ratio(full.total_coverage!));
  // Every returned digit stays in the title.
  expect(
    screen
      .getByText("Net leverage", { selector: "dt" })
      .nextElementSibling!.firstElementChild!.getAttribute("title"),
  ).toBe(String(full.net_leverage));
  expect(
    screen.getByText(
      `${full.ltm_periods_found} of ${full.ltm_periods_required} periods found`,
    ),
  ).toBeTruthy();
  expect(screen.queryByText("Undefined")).toBeNull();
});

it("lists the tranches in Rust's order with the figures through each class", () => {
  render(<CapitalStructureReport {...props} summary={full} />);
  const table = screen.getByRole("table", {
    name: `Debt stack at ${full.as_of} (${full.currency})`,
  });
  const rows = within(table).getAllByRole("row").slice(1);
  expect(
    rows.map((row) => within(row).getByRole("rowheader").textContent),
  ).toEqual(full.tranches.map((tranche) => tranche.id));
  // Two pari passu first-lien tranches ahead of the second lien.
  expect(new Set(full.tranches.map((tranche) => tranche.rank)).size).toBe(2);
  full.tranches.forEach((tranche, index) => {
    expect(
      within(rows[index]!)
        .getAllByRole("cell")
        .map((cell) => cell.textContent),
    ).toEqual([
      "Bond",
      tranche.payment_class,
      String(tranche.rank),
      money(tranche.balance),
      share(tranche.share_of_debt!),
      money(tranche.cumulative_debt),
      ratio(tranche.leverage!),
      ratio(tranche.interest_coverage!),
      money(tranche.ltm_interest!),
      tranche.maturity,
    ]);
    expect(
      within(rows[index]!)
        .getAllByRole("cell")
        .slice(4, 8)
        .map((cell) => cell.firstElementChild?.getAttribute("title")),
    ).toEqual([
      String(tranche.share_of_debt),
      undefined,
      String(tranche.leverage),
      String(tranche.interest_coverage),
    ]);
  });
  expect(
    screen.getByRole("img", { name: `Maturity wall at ${full.as_of}` }),
  ).toBeTruthy();
});

it("draws the maturity wall from each returned bucket, with no sums", async () => {
  const bars = maturityBars(full);
  expect(bars.map((bar) => bar.year)).toEqual(
    full.maturity_wall.map((bucket) => String(bucket.year)),
  );
  full.maturity_wall.forEach((bucket, index) => {
    expect(bars[index]).toMatchObject({
      value: Number(bucket.amount.amount),
      amount: bucket.amount,
      share: bucket.share_of_debt,
    });
  });
  const source = await readFile(
    resolve(
      import.meta.dirname,
      "../../../registry/statements/components/capital-structure-report/capital-structure-report.tsx",
    ),
    "utf8",
  );
  expect(source).not.toMatch(/reduce\(|\+=|Big\(/);
});

it("shows Rust's null ratios of a short LTM window as Undefined, beside the periods found", () => {
  const short = summaryOf("shortWindow");
  expect(short.ltm_periods_found).toBeLessThan(short.ltm_periods_required);
  expect(short.total_leverage).toBeNull();
  render(<CapitalStructureReport {...props} summary={short} />);
  const value = (label: string) =>
    screen.getByText(label, { selector: "dt" }).nextElementSibling!;
  for (const label of [
    "LTM EBITDA",
    "Total leverage",
    "Net leverage",
    "Total coverage",
  ])
    expect(value(label).textContent).toBe("Undefined");
  expect(value("Net debt").textContent).toBe(money(short.net_debt!));
  expect(
    screen.getByText(
      `${short.ltm_periods_found} of ${short.ltm_periods_required} periods found`,
    ),
  ).toBeTruthy();
  const table = screen.getByRole("table", {
    name: `Debt stack at ${short.as_of} (${short.currency})`,
  });
  expect(within(table).getAllByText("Undefined")).toHaveLength(
    // Leverage, interest coverage and LTM interest of every tranche.
    short.tranches.length * 3,
  );
  expect(screen.queryByText("NaN")).toBeNull();
});

it("shows a live NaN as Undefined, never NaN", () => {
  const live = {
    ...full,
    total_leverage: Number.NaN,
    tranches: full.tranches.map((tranche) => ({
      ...tranche,
      leverage: Number.NaN,
    })),
  } as CapitalStructureSummary;
  render(<CapitalStructureReport {...props} summary={live} />);
  expect(
    screen.getByText("Total leverage", { selector: "dt" }).nextElementSibling!
      .textContent,
  ).toBe("Undefined");
  expect(screen.queryByText("NaN")).toBeNull();
});

it("shows no cash as Undefined and negative net debt and leverage as returned", () => {
  const noCash = summaryOf("noCash");
  const { rerender } = render(
    <CapitalStructureReport {...props} summary={noCash} />,
  );
  const value = (label: string) =>
    screen.getByText(label, { selector: "dt" }).nextElementSibling!.textContent;
  expect(value("Cash")).toBe("Undefined");
  expect(value("Net leverage")).toBe("Undefined");
  expect(value("Total leverage")).toBe(ratio(noCash.total_leverage!));
  const netCash = summaryOf("netCash");
  expect(netCash.net_leverage).toBeLessThan(0);
  rerender(<CapitalStructureReport {...props} summary={netCash} />);
  expect(value("Net debt")).toBe(money(netCash.net_debt!));
  expect(value("Net debt")!.startsWith("-")).toBe(true);
  expect(value("Net leverage")).toBe(ratio(netCash.net_leverage!));
});

it("shows native errors and loading as such", () => {
  const { rerender } = render(
    <CapitalStructureReport {...props} summary={null} loading />,
  );
  expect(screen.getByRole("status").textContent).toBe(
    "Summarizing the capital structure…",
  );
  rerender(<CapitalStructureReport {...props} summary={undefined} />);
  expect(screen.getByRole("status").textContent).toBe(
    "No capital structure summary available.",
  );
  for (const { message } of Object.values(fixture.errors)) {
    rerender(
      <CapitalStructureReport {...props} summary={full} error={message} />,
    );
    expect(screen.getByRole("alert").textContent).toBe(message);
    expect(screen.queryByRole("table")).toBeNull();
  }
});

/** Submissions the live form has made in the current test. */
let submitted = 0;
function Live() {
  const [configJson, setConfigJson] = useState<string | null>(null);
  const summary = useCapitalStructureSummary(
    configJson === null ? null : { resultJson, modelJson, configJson },
  );
  return (
    <CapitalStructureReport
      defaultConfig={fixture.cases.full.config}
      nodeIds={nodeIds}
      periodIds={periodIds}
      onSubmit={(json) => {
        submitted += 1;
        setConfigJson(json);
      }}
      summary={summary.data}
      loading={summary.isFetching}
      error={summary.error?.message}
    />
  );
}

it(
  "edits the period and nodes over the model and summarizes through the worker",
  async () => {
    render(
      <FinstackQueryProvider>
        <Live />
      </FinstackQueryProvider>,
    );
    expect(
      (
        screen.getByRole("combobox", {
          name: "EBITDA node",
        }) as HTMLInputElement
      ).value,
    ).toBe("ebitda");
    expect(
      screen.getByRole("combobox", { name: "Period" }).textContent,
    ).toContain(full.as_of);
    // The config has no native validator (Rust validates it on the call), so
    // the click repeats only until the form's pass-through validation settles.
    submitted = 0;
    const submit = screen.getByRole("button", { name: "Summarize" });
    await waitFor(
      () => {
        fireEvent.click(submit);
        expect(submitted).toBeGreaterThan(0);
      },
      { timeout: budget(10000) },
    );
    const table = await screen.findByRole(
      "table",
      { name: `Debt stack at ${full.as_of} (${full.currency})` },
      { timeout: budget(10000) },
    );
    expect(within(table).getAllByRole("rowheader")[0]!.textContent).toBe(
      full.tranches[0]!.id,
    );
  },
  budget(30000),
);

it("shows the native rejection of a node the results lack", async () => {
  function Missing() {
    const summary = useCapitalStructureSummary({
      resultJson,
      modelJson,
      configJson: JSON.stringify(fixture.errors.missingNode.config),
    });
    return (
      <CapitalStructureReport
        {...props}
        summary={summary.data}
        error={summary.error?.message}
      />
    );
  }
  render(
    <FinstackQueryProvider>
      <Missing />
    </FinstackQueryProvider>,
  );
  expect(
    (await screen.findByRole("alert", undefined, { timeout: budget(10000) }))
      .textContent,
  ).toBe(fixture.errors.missingNode.message);
});

it("keys the query on the result, model and config", () => {
  const client = { call: vi.fn() } as unknown as FinstackClient;
  const request = { resultJson: "{}", modelJson: "{}", configJson: "{}" };
  const key = hashKey(
    capitalStructureSummaryOptions(client, 1, request).queryKey,
  );
  expect(
    hashKey(capitalStructureSummaryOptions(client, 2, request).queryKey),
  ).not.toBe(key);
  for (const field of Object.keys(request))
    expect(
      hashKey(
        capitalStructureSummaryOptions(client, 1, {
          ...request,
          [field]: "changed",
        }).queryKey,
      ),
    ).not.toBe(key);
  expect(capitalStructureSummaryOptions(client, 1, null).enabled).toBe(false);
});
