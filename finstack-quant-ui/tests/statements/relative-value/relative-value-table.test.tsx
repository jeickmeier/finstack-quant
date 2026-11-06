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
  relativeValueReportOptions,
  useRelativeValueReport,
} from "@/hooks/statements/use-relative-value-report/use-relative-value-report";
import {
  RelativeValueTable,
  fittedLine,
  percentileBars,
  scatterPoints,
} from "@/components/finstack/statements/components/relative-value-table/relative-value-table";
import {
  unwrap,
  type CompanyMetrics,
  type RelativeValueReport,
  type RelativeValueReportRequest,
} from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
import type { RelativeValueReportConfigWire } from "../../../src/generated/types/relative_value_report_config";

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
const harness = useWorkerHarness(["relativeValueReport"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
const subject = fixture.subject as CompanyMetrics;
const universe = fixture.universe as CompanyMetrics[];
const subjectJson = JSON.stringify(subject);
const universeJson = JSON.stringify(universe);
const reportOf = (name: keyof typeof fixture.cases) =>
  fixture.cases[name].report as unknown as RelativeValueReport;
const full = reportOf("full");
const props = { subject, universe, onSubmit: () => {} };
const number = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 4,
  signDisplay: "negative",
}).format;
const percent = new Intl.NumberFormat("en-US", {
  style: "percent",
  maximumFractionDigits: 1,
}).format;
const value = (scope: HTMLElement, label: string) =>
  within(scope).getByText(label, { selector: "dt" }).nextElementSibling!
    .textContent;

it("returns the fixture's reports and the typo rejection through the worker", async () => {
  for (const { config, report } of Object.values(fixture.cases))
    expect(
      unwrap(
        await harness.proxy.relativeValueReport({
          subjectJson,
          universeJson,
          configJson: JSON.stringify(config),
        }),
      ),
    ).toEqual(report);
  const rejected = await harness.proxy.relativeValueReport({
    subjectJson,
    universeJson,
    configJson: JSON.stringify(fixture.errors.typo.config),
  });
  expect(rejected.ok).toBe(false);
});

it("shows the subject, the peer counts and the composite score", () => {
  render(<RelativeValueTable {...props} report={full} />);
  const headline = screen.getByLabelText(
    `Relative value of ${full.subject_id}`,
  );
  expect(value(headline, "Subject id")).toBe(full.subject_id);
  expect(value(headline, "Period basis")).toBe("LTM");
  expect(value(headline, "Peer count")).toBe(String(full.peer_count));
  expect(value(headline, "Excluded count")).toBe(String(full.excluded_count));
  expect(value(headline, "Composite score")).toBe(
    number(full.composite!.composite_score),
  );
  expect(value(headline, "Confidence")).toBe(
    number(full.composite!.confidence),
  );
  // The partial filter kept the eight energy peers and dropped the utilities.
  expect([full.peer_count, full.excluded_count]).toEqual([8, 2]);
  expect(screen.queryByText(/No composite score/)).toBeNull();
});

it("lists the comps table with the subject first, marked from is_subject", () => {
  render(<RelativeValueTable {...props} report={full} />);
  const table = screen.getByRole("table", {
    name: "Comparable companies, LTM",
  });
  const header = within(table).getAllByRole("columnheader");
  expect(header.map((cell) => cell.textContent)).toEqual([
    "Id",
    ...full.multiples.map((row) => labelFor(row.multiple)),
  ]);
  expect(header.map((cell) => cell.textContent)).toContain("EV EBITDA");
  const rows = within(table).getAllByRole("row").slice(1);
  expect(rows).toHaveLength(full.companies.length);
  full.companies.forEach((company, index) => {
    const row = rows[index]!;
    expect(within(row).getByRole("rowheader").textContent).toBe(
      company.is_subject ? `${company.id}Subject` : company.id,
    );
    expect(
      within(row)
        .getAllByRole("cell")
        .map((cell) => cell.textContent),
    ).toEqual(
      company.values.map((entry) =>
        entry == null ? "Undefined" : number(entry),
      ),
    );
  });
  expect(full.companies.filter((company) => company.is_subject)).toHaveLength(
    1,
  );
  // GINKGO reports no earnings, so Rust returns no P/E for it.
  expect(within(table).getAllByText("Undefined").length).toBeGreaterThan(0);
});

it("shows each multiple's subject value, percentile, z-score and peer statistics", () => {
  render(<RelativeValueTable {...props} report={full} />);
  for (const row of full.multiples) {
    const card = screen.getByRole("article", { name: labelFor(row.multiple) });
    expect(value(card, "Subject value")).toBe(number(row.subject_value!));
    expect(value(card, "Percentile")).toBe(percent(row.percentile!));
    expect(value(card, "Z score")).toBe(
      row.z_score == null ? "Undefined" : number(row.z_score),
    );
    const stats = within(card).getByLabelText(
      `${labelFor(row.multiple)} peer statistics`,
    );
    expect(value(stats, "Count")).toBe(String(row.peer_stats!.count));
    expect(value(stats, "Median")).toBe(number(row.peer_stats!.median));
    expect(value(stats, "Std dev")).toBe(number(row.peer_stats!.std_dev));
  }
  expect(
    screen.getByRole("img", {
      name: `Percentile of ${full.subject_id} by multiple`,
    }),
  ).toBeTruthy();
});

it("draws each percentile bar at Rust's percentile, with no arithmetic", async () => {
  const bars = percentileBars(full);
  expect(bars).toEqual(
    full.multiples.map((row) => ({
      label: labelFor(row.multiple),
      percentile: row.percentile,
    })),
  );
  const source = await readFile(
    resolve(
      import.meta.dirname,
      "../../../registry/statements/components/relative-value-table/relative-value-table.tsx",
    ),
    "utf8",
  );
  expect(source).not.toMatch(/reduce\(|\+=|Big\(|intercept|slope \*/);
});

it("plots the regression points with Rust's subject mark and the line through Rust's fitted values", () => {
  const [regression] = full.dimensions;
  const points = scatterPoints(regression!);
  expect(points.map((point) => point.company_id)).toEqual(
    regression!.points.map((point) => point.company_id),
  );
  expect(points.map((point) => point.role)).toEqual(
    regression!.points.map((point) => (point.is_subject ? "Subject" : "Peer")),
  );
  expect(
    points.filter((point) => point.role === "Subject").map((p) => p.company_id),
  ).toEqual([full.subject_id]);
  const line = fittedLine(regression!);
  expect(line.map((point) => point.fitted_y)).toEqual(
    [...regression!.points]
      .sort((a, b) => (a.x < b.x ? -1 : 1))
      .map((point) => point.fitted_y),
  );
  // HAZEL reports no leverage: in the Y statistics, not on the scatter.
  expect(points.map((point) => point.company_id)).not.toContain("HAZEL");
  expect(regression!.peer_stats!.count).toBe(full.peer_count);

  render(<RelativeValueTable {...props} report={full} />);
  const panel = screen.getByRole("region", { name: regression!.label });
  expect(value(panel, "Signal")).toBe(number(regression!.signal!));
  expect(value(panel, "Z score")).toBe(number(regression!.z_score!));
  expect(value(panel, "R squared")).toBe(
    number(regression!.regression!.r_squared),
  );
  expect(value(panel, "Fitted value")).toBe(
    number(regression!.regression!.fitted_value),
  );
  expect(value(panel, "Direction")).toBe("Higher is cheap");
  expect(
    within(panel).getByRole("img", {
      name: `${regression!.label}: OAS bp on Leverage`,
    }),
  ).toBeTruthy();
  expect(within(panel).queryByText(/Excluded from the composite/)).toBeNull();
});

it("shows an excluded dimension's reason with no signal or scatter of the subject", () => {
  const report = reportOf("excludedDimension");
  const excluded = report.dimensions.find(
    (dimension) => dimension.excluded_reason,
  )!;
  expect(excluded.excluded_reason).toBe("no_subject_x");
  render(<RelativeValueTable {...props} report={report} />);
  const panel = screen.getByRole("region", { name: excluded.label });
  expect(
    within(panel).getByText(/Excluded from the composite: No subject x\./)
      .textContent,
  ).toContain("The subject has no finite value of the X metric.");
  expect(value(panel, "Signal")).toBe("Undefined");
  expect(value(panel, "Subject x")).toBe("Undefined");
  expect(
    scatterPoints(excluded).some((point) => point.role === "Subject"),
  ).toBe(false);
  // The composite still scores on the two usable dimensions.
  expect(report.composite).not.toBeNull();
});

it("shows Rust's reason for no composite when too few peers remain", () => {
  const report = reportOf("tooFewPeers");
  expect(report.peer_count).toBe(1);
  expect(report.composite).toBeNull();
  render(<RelativeValueTable {...props} report={report} />);
  const headline = screen.getByLabelText(
    `Relative value of ${report.subject_id}`,
  );
  expect(value(headline, "Composite score")).toBe("Undefined");
  expect(value(headline, "Period basis")).toBe("FY2025E");
  expect(
    screen.getByText(
      new RegExp(
        `No composite score: ${labelFor(report.composite_excluded_reason!)}`,
      ),
    ),
  ).toBeTruthy();
  for (const dimension of report.dimensions)
    expect(
      within(screen.getByRole("region", { name: dimension.label })).getByText(
        new RegExp(
          `Excluded from the composite: ${labelFor(dimension.excluded_reason!)}`,
        ),
      ),
    ).toBeTruthy();
  expect(
    screen.getByRole("table", { name: "Comparable companies, FY2025E" }),
  ).toBeTruthy();
});

it("shows Rust's -0.0 signal as 0 and a live NaN as Undefined", () => {
  const [regression, multiple] = [full.dimensions[0]!, full.multiples[0]!];
  const live = {
    ...full,
    multiples: [
      { ...multiple, z_score: Number.NaN },
      ...full.multiples.slice(1),
    ],
    dimensions: [
      { ...regression, signal: -0, z_score: Number.NaN },
      ...full.dimensions.slice(1),
    ],
    companies: full.companies.map((company) => ({
      ...company,
      values: company.values.map(() => Number.NaN),
    })),
  } as RelativeValueReport;
  render(<RelativeValueTable {...props} report={live} />);
  const panel = screen.getByRole("region", { name: regression.label });
  expect(value(panel, "Signal")).toBe("0");
  expect(value(panel, "Z score")).toBe("Undefined");
  const card = screen.getByRole("article", {
    name: labelFor(multiple.multiple),
  });
  expect(value(card, "Z score")).toBe("Undefined");
  expect(screen.queryByText("NaN")).toBeNull();
  expect(screen.queryByText("-0")).toBeNull();
  // The default display rounding never reads "-0" for a tiny negative either.
  render(
    <RelativeValueTable
      {...props}
      report={{
        ...full,
        dimensions: [{ ...regression, signal: -0.00001 }],
      }}
    />,
  );
  expect(screen.queryByText("-0")).toBeNull();
});

it("shows native errors and loading as such", () => {
  const { rerender } = render(
    <RelativeValueTable {...props} report={null} loading />,
  );
  expect(screen.getByRole("status").textContent).toBe(
    "Comparing the subject with its peers…",
  );
  rerender(<RelativeValueTable {...props} report={undefined} />);
  expect(screen.getByRole("status").textContent).toBe(
    "No relative value report available.",
  );
  rerender(
    <RelativeValueTable
      {...props}
      report={full}
      error={fixture.errors.typo.message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(
    fixture.errors.typo.message,
  );
  expect(fixture.errors.typo.message).toContain("gics_sector");
  expect(screen.queryByRole("table")).toBeNull();
});

/** Requests the live form has submitted in the current test. */
let submitted: RelativeValueReportRequest[] = [];
function Live() {
  const [request, setRequest] = useState<RelativeValueReportRequest | null>(
    null,
  );
  const report = useRelativeValueReport(request);
  return (
    <RelativeValueTable
      {...props}
      defaultConfig={
        fixture.cases.full.config as unknown as RelativeValueReportConfigWire
      }
      onSubmit={(next) => {
        submitted.push(next);
        setRequest(next);
      }}
      report={report.data}
      loading={report.isFetching}
      error={report.error?.message}
    />
  );
}

it(
  "edits the config over the generated contract and compares through the worker",
  async () => {
    render(
      <FinstackQueryProvider>
        <Live />
      </FinstackQueryProvider>,
    );
    submitted = [];
    const submit = screen.getByRole("button", { name: "Compare" });
    // The config has no native validator (Rust validates it on the call), so
    // the click repeats only until the form's pass-through validation settles.
    await waitFor(
      () => {
        fireEvent.click(submit);
        expect(submitted.length).toBeGreaterThan(0);
      },
      { timeout: budget(10000) },
    );
    const [request] = submitted;
    expect(request!.subjectJson).toBe(subjectJson);
    expect(request!.universeJson).toBe(universeJson);
    expect(JSON.parse(request!.configJson).filter.gics_sectors).toEqual([
      "Energy",
    ]);
    const table = await screen.findByRole(
      "table",
      { name: "Comparable companies, LTM" },
      { timeout: budget(10000) },
    );
    expect(within(table).getAllByRole("rowheader")[0]!.textContent).toBe(
      `${full.subject_id}Subject`,
    );
  },
  budget(30000),
);

it("shows the native rejection of a misspelt filter field", async () => {
  function Typo() {
    const report = useRelativeValueReport({
      subjectJson,
      universeJson,
      configJson: JSON.stringify(fixture.errors.typo.config),
    });
    return (
      <RelativeValueTable
        {...props}
        report={report.data}
        error={report.error?.message}
      />
    );
  }
  render(
    <FinstackQueryProvider>
      <Typo />
    </FinstackQueryProvider>,
  );
  expect(
    (await screen.findByRole("alert", undefined, { timeout: budget(10000) }))
      .textContent,
  ).toBe(fixture.errors.typo.message);
});

it("keys the query on the subject, universe and config", () => {
  const client = { call: vi.fn() } as unknown as FinstackClient;
  const request = { subjectJson: "{}", universeJson: "[]", configJson: "{}" };
  const key = hashKey(relativeValueReportOptions(client, 1, request).queryKey);
  expect(
    hashKey(relativeValueReportOptions(client, 2, request).queryKey),
  ).not.toBe(key);
  for (const field of Object.keys(request))
    expect(
      hashKey(
        relativeValueReportOptions(client, 1, {
          ...request,
          [field]: "changed",
        }).queryKey,
      ),
    ).not.toBe(key);
  expect(relativeValueReportOptions(client, 1, null).enabled).toBe(false);
});
