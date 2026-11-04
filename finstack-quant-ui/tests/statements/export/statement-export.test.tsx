// @vitest-environment jsdom
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
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  statementResultTableOptions,
  useStatementResultTable,
} from "@/hooks/statements/use-statement-result-table/use-statement-result-table";
import {
  StatementExport,
  statementTableCsv,
  type StatementTableLayout,
} from "@/components/finstack/statements/components/statement-export/statement-export";
import { unwrap } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import analystResult from "../../../src/fixtures/statements/analyst-result.json";
import fixture from "./cases.json";

vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness([
  "statementResultToTableWide",
  "statementResultToTableLong",
]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
const resultJson = JSON.stringify(analystResult);
const wide = fixture.analyst.wide as unknown as TableEnvelopeWire;
const long = fixture.analyst.long as unknown as TableEnvelopeWire;
/** RFC 4180 records of CSV text, independent of the code under test. */
function parseCsv(text: string): string[][] {
  const records: string[][] = [];
  let record: string[] = [];
  let field = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index]!;
    if (quoted) {
      if (char === '"' && text[index + 1] === '"') {
        field += '"';
        index += 1;
      } else if (char === '"') quoted = false;
      else field += char;
    } else if (char === '"') quoted = true;
    else if (char === ",") {
      record.push(field);
      field = "";
    } else if (char === "\r" && text[index + 1] === "\n") {
      record.push(field);
      records.push(record);
      record = [];
      field = "";
      index += 1;
    } else field += char;
  }
  return records;
}
const column = (table: TableEnvelopeWire, name: string) =>
  table.columns.find((entry) => entry.name === name)!.data
    .values as readonly unknown[];

it("writes exactly the native cells, with a header of native names", () => {
  for (const table of [wide, long]) {
    const records = parseCsv(statementTableCsv(table));
    expect(records[0]).toEqual(table.columns.map((entry) => entry.name));
    expect(records).toHaveLength(table.row_count + 1);
    table.columns.forEach((entry, index) => {
      expect(records.slice(1).map((record) => record[index])).toEqual(
        (entry.data.values as readonly unknown[]).map((cell) =>
          cell === null ? "" : String(cell),
        ),
      );
    });
  }
});

it("ends every record in CRLF", () => {
  const text = statementTableCsv(wide);
  expect(text.endsWith("\r\n")).toBe(true);
  expect(text.split("\r\n")).toHaveLength(wide.row_count + 2);
  expect(text.replaceAll("\r\n", "")).not.toMatch(/[\r\n]/);
});

it("writes a number as its String form, with no rounding", () => {
  const values = column(wide, "interest_coverage");
  const rows = statementTableCsv(wide).split("\r\n");
  expect(String(values[0])).toMatch(/\.\d{10,}/);
  expect(rows[1]).toContain(`,${String(values[0])},`);
});

it("quotes a field that holds a comma or a quote, per RFC 4180", () => {
  const csv = statementTableCsv(
    fixture.quoted.wide as unknown as TableEnvelopeWire,
  );
  expect(csv.split("\r\n")[0]).toContain('"adjusted, ""core"" revenue"');
  const header = parseCsv(csv)[0]!;
  expect(header).toContain(fixture.quotedId);
  const long = statementTableCsv(
    fixture.quoted.long as unknown as TableEnvelopeWire,
  );
  expect(parseCsv(long).some((record) => record[0] === fixture.quotedId)).toBe(
    true,
  );
});

it("delivers a missing value as NaN through the worker, written empty and shown undefined", async () => {
  const live = unwrap(
    await harness.proxy.statementResultToTableWide(resultJson),
  ) as unknown as TableEnvelopeWire;
  const index = live.columns.findIndex(
    (entry) => entry.name === "revenue_growth_qoq",
  );
  const [cell] = live.columns[index]!.data.values as readonly number[];
  expect(Number.isNaN(cell)).toBe(true);
  // The same empty field the JSON fixture's null gives.
  expect(parseCsv(statementTableCsv(live))).toEqual(
    parseCsv(statementTableCsv(wide)),
  );
  render(
    <StatementExport layout="wide" onLayoutChange={() => {}} table={live} />,
  );
  const na = screen.getAllByText("Undefined");
  expect(na[0]!.className).toContain("finstack-na");
  expect(screen.queryByText("NaN")).toBeNull();
});

it("shows a JSON null as undefined, and a zero as a value", () => {
  render(
    <StatementExport layout="wide" onLayoutChange={() => {}} table={wide} />,
  );
  const nulls = wide.columns.flatMap((entry) =>
    (entry.data.values as readonly unknown[]).filter((v) => v === null),
  );
  expect(screen.getAllByText("Undefined")).toHaveLength(nulls.length);
  expect(column(wide, "debt_draws")[0]).toBe(0);
  const row = screen.getAllByRole("row")[1]!;
  expect(within(row).getAllByText("0").length).toBeGreaterThan(0);
});

it("downloads the shown table under the layout's name", async () => {
  const blobs: Blob[] = [];
  vi.stubGlobal(
    "URL",
    Object.assign(URL, {
      createObjectURL: (blob: Blob) => {
        blobs.push(blob);
        return "blob:csv";
      },
      revokeObjectURL: () => {},
    }),
  );
  const names: string[] = [];
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    names.push(this.download);
  });
  render(
    <StatementExport layout="long" onLayoutChange={() => {}} table={long} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Download CSV" }));
  expect(names).toEqual(["statement-long.csv"]);
  expect(blobs[0]!.type).toBe("text/csv;charset=utf-8");
  const text = await new Promise<string>((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.readAsText(blobs[0]!);
  });
  expect(text).toBe(statementTableCsv(long));
});

it("offers no download without a table, while loading, or on an error", () => {
  const button = () =>
    screen.getByRole("button", { name: "Download CSV" }) as HTMLButtonElement;
  const { rerender } = render(
    <StatementExport layout="wide" onLayoutChange={() => {}} table={null} />,
  );
  expect(button().disabled).toBe(true);
  rerender(
    <StatementExport
      layout="wide"
      onLayoutChange={() => {}}
      table={wide}
      loading
    />,
  );
  expect(button().disabled).toBe(true);
  rerender(
    <StatementExport
      layout="wide"
      onLayoutChange={() => {}}
      table={wide}
      error="native failure"
    />,
  );
  expect(button().disabled).toBe(true);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});

function Live() {
  const [layout, setLayout] = useState<StatementTableLayout>("wide");
  const table = useStatementResultTable({
    resultJson,
    layout,
  });
  return (
    <StatementExport
      layout={layout}
      onLayoutChange={setLayout}
      table={(table.data as unknown as TableEnvelopeWire) ?? null}
      loading={table.isFetching}
      error={table.error?.message}
    />
  );
}

it("switches layout through the worker", async () => {
  render(
    <FinstackQueryProvider>
      <Live />
    </FinstackQueryProvider>,
  );
  const caption = { timeout: budget(10000) };
  expect(
    await screen.findByText(
      "Statement result, wide layout",
      undefined,
      caption,
    ),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "Long" }));
  expect(
    await screen.findByText(
      "Statement result, long layout",
      undefined,
      caption,
    ),
  ).toBeTruthy();
  await waitFor(() =>
    expect(screen.getAllByRole("row").length).toBeGreaterThan(wide.row_count),
  );
});

it("shows the native rejection of a malformed result", async () => {
  function Broken() {
    const table = useStatementResultTable({
      resultJson: "{}",
      layout: "wide",
    });
    return (
      <StatementExport
        layout="wide"
        onLayoutChange={() => {}}
        table={null}
        error={table.error?.message}
      />
    );
  }
  render(
    <FinstackQueryProvider>
      <Broken />
    </FinstackQueryProvider>,
  );
  expect(
    (await screen.findByRole("alert", undefined, { timeout: budget(10000) }))
      .textContent?.length,
  ).toBeGreaterThan(0);
});

it("keys the query on the result and layout", () => {
  const client = { call: vi.fn() } as unknown as FinstackClient;
  const key = (layout: StatementTableLayout) =>
    statementResultTableOptions(client, 1, { resultJson: "{}", layout })
      .queryKey;
  expect(key("wide")).not.toEqual(key("long"));
  expect(statementResultTableOptions(client, 1, null).enabled).toBe(false);
});
