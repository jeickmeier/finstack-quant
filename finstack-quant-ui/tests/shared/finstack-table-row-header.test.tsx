// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import {
  FinstackTable,
  type CellActivation,
} from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
const rows = [
  { id: "r1", line: "Revenue", value: "120.0" },
  { id: "r2", line: "Opex", value: "45.0" },
];
type Row = (typeof rows)[number];
const columns = [
  {
    id: "line",
    header: "Line item",
    accessorFn: (row: Row) => row.line,
    meta: { className: "", rowHeader: true },
  },
  {
    id: "value",
    header: "Value",
    accessorFn: (row: Row) => row.value,
    meta: { className: "text-right" },
  },
];
afterEach(cleanup);

it("renders a rowHeader column's body cells as row headers styled like plain cells", () => {
  render(
    <FinstackTable
      caption="Scenario values"
      data={rows}
      getRowId={(row) => row.id}
      columns={columns}
    />,
  );
  const table = screen.getByRole("table", { name: "Scenario values" });
  expect(
    within(table)
      .getAllByRole("columnheader")
      .map((header) => header.textContent),
  ).toEqual(["Line item", "Value"]);
  const headers = within(table).getAllByRole("rowheader");
  expect(headers.map((header) => header.textContent)).toEqual([
    "Revenue",
    "Opex",
  ]);
  for (const header of headers) {
    expect(header.tagName).toBe("TH");
    expect(header.getAttribute("scope")).toBe("row");
    // The stock header height and weight give way to the cell row height and normal weight.
    for (const kept of ["h-[var(--row-height)]", "px-2", "py-0", "font-normal"])
      expect(header.classList.contains(kept)).toBe(true);
    for (const dropped of ["h-10", "font-medium"])
      expect(header.classList.contains(dropped)).toBe(false);
  }
  expect(
    within(table)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual(["120.0", "45.0"]);
  expect(
    within(table).getAllByRole("cell")[0].classList.contains("font-normal"),
  ).toBe(false);
});

it("keeps linked selection and activation on row-header cells", async () => {
  const activations: CellActivation<Row>[] = [];
  const row = vi.fn();
  const proposals: (string | null)[] = [];
  function Linked() {
    const [key, setKey] = useState<string | null>(null);
    const link = useLinkedSelection({
      selectedKey: key,
      onSelectedKeyChange(next) {
        proposals.push(next);
        setKey(next);
      },
    });
    return (
      <FinstackTable
        caption="Linked scenario values"
        data={rows}
        getRowId={(line) => line.id}
        columns={columns}
        link={link}
        getRowKey={(line) => `row:${line.id}`}
        getCellKey={(line, column) => `cell:${column}:${line.id}`}
        getActiveCell={(selected) => {
          const [kind, columnId, rowId] = selected.split(":");
          return kind === "cell" ? { rowId, columnId } : null;
        }}
        onRowActivate={row}
        onCellActivate={(cell) => activations.push(cell)}
      />
    );
  }
  const user = userEvent.setup();
  const { container } = render(<Linked />);
  const [revenue] = screen.getAllByRole("rowheader");
  expect(revenue.getAttribute("tabindex")).toBe("0");
  await user.click(revenue);
  expect(proposals).toEqual(["cell:line:r1"]);
  expect(activations).toEqual([
    { row: rows[0], rowId: "r1", columnId: "line", value: "Revenue" },
  ]);
  expect(row).not.toHaveBeenCalled();
  expect(revenue.getAttribute("aria-selected")).toBe("true");
  expect(revenue.getAttribute("data-linked")).toBe("true");
  revenue.focus();
  await user.keyboard("{Enter}");
  expect(activations).toHaveLength(2);
  expect(row).not.toHaveBeenCalled();
  expect((await axe(container)).violations).toEqual([]);
});
