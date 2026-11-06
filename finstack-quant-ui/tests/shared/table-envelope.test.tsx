// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { axe } from "./axe";
import { TableEnvelope } from "@/components/finstack/shared/table/table-envelope/table-envelope";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import analystPlSummary from "../../src/fixtures/statements/analyst-pl-summary.json";

afterEach(cleanup);

const plSummary = analystPlSummary as unknown as TableEnvelopeWire;

function bodyRows() {
  const [, body] = screen.getAllByRole("rowgroup");
  return within(body!).getAllByRole("row");
}
function cellTexts(row: HTMLElement) {
  return within(row)
    .getAllByRole("cell")
    .map((cell) => cell.textContent);
}

it("shows a native plSummaryReport envelope in its own column and row order", async () => {
  const { container } = render(
    <TableEnvelope envelope={plSummary} caption="P&L summary" />,
  );
  expect(
    screen.getAllByRole("columnheader").map((header) => header.textContent),
  ).toEqual(["Line item", "Period", "Value"]);
  const rows = bodyRows();
  expect(rows).toHaveLength(plSummary.row_count);
  const [lineItem, period, value] = plSummary.columns.map(
    (column) => column.data.values,
  );
  rows.forEach((row, index) => {
    const [line, quarter] = cellTexts(row);
    expect(line).toBe(lineItem![index]);
    expect(quarter).toBe(period![index]);
  });
  // Every supplied digit is kept and grouped; nothing is rounded.
  expect(value![0]).toBe(208300000);
  expect(cellTexts(rows[0]!)[2]).toBe("208,300,000");
  expect((await axe(container)).violations).toEqual([]);
});

it("keeps native column order, labels headers and aligns only numeric columns", () => {
  const envelope: TableEnvelopeWire = {
    row_count: 2,
    columns: [
      {
        name: "zeta_pv",
        data: { type: "nullable_float64", values: [1234.5, null] },
      },
      {
        name: "metric",
        data: { type: "nullable_string", values: [null, "ytm"] },
        role: "dimension",
      },
      {
        name: "count",
        data: { type: "int64", values: [9007199254740993n as never, 0] },
      },
      { name: "alpha", data: { type: "u_int32", values: [7, 1e3] } },
    ],
  };
  render(<TableEnvelope envelope={envelope} caption="Supplied envelope" />);
  const headers = screen.getAllByRole("columnheader");
  expect(headers.map((header) => header.textContent)).toEqual([
    "Zeta PV",
    "Metric",
    "Count",
    "Alpha",
  ]);
  expect(
    headers.map((header) => header.className.includes("text-right")),
  ).toEqual([true, false, true, true]);
  const rows = bodyRows();
  expect(rows.map(cellTexts)).toEqual([
    ["1,234.5", "—", "9,007,199,254,740,993", "7"],
    ["—", "ytm", "0", "1,000"],
  ]);
});

it("shows the supplied empty state for an envelope with no rows", () => {
  render(
    <TableEnvelope
      envelope={{ row_count: 0, columns: [] }}
      caption="Empty envelope"
      emptyState="No native rows"
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("No native rows");
});
