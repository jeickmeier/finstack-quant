// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import {
  FinstackTable,
  type CellActivation,
} from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { tableSelection } from "@/components/finstack/shared/table/finstack-table/selection";
import { useLinkedSelection } from "@/hooks/shared/use-linked-selection/use-linked-selection";
const rows = [
  { id: "a_b", series: "alpha", value: 9007199254740993n },
  { id: "a", series: "beta", value: 9007199254740995n },
];
type Row = (typeof rows)[number];
const getRowId = (row: Row) => row.id;
const getRowKey = (row: Row) => `series:${row.series}`;
const getCellKey = (row: Row, column: string) =>
  column === "value" ? `point:${row.series}` : null;
const getActiveCell = (key: string) => {
  const row = rows.find((row) => getCellKey(row, "value") === key);
  return row ? { rowId: row.id, columnId: "value" } : null;
};
afterEach(cleanup);
it("derives native state, requires explicit addresses, and resolves functional updaters", () => {
  const select = vi.fn();
  const props = {
    link: { selectedKey: "point:alpha", select, clear() {} },
    getRowKey,
    getCellKey,
  };
  expect(
    tableSelection(rows, getRowId, ["value"], props).state.cellSelection,
  ).toEqual([]);
  const adapter = tableSelection(rows, getRowId, ["value"], {
    ...props,
    getActiveCell,
  });
  expect(adapter.state.cellSelection).toEqual([
    {
      anchorRowId: "a_b",
      focusRowId: "a_b",
      anchorColumnId: "value",
      focusColumnId: "value",
    },
  ]);
  adapter.onCellSelectionChange((old) => {
    expect(old).toBe(adapter.state.cellSelection);
    return [{ ...old[0], anchorRowId: "a", focusRowId: "a" }];
  });
  expect(select).toHaveBeenLastCalledWith("point:beta");
  adapter.onRowSelectionChange((old) => {
    expect(old).toEqual({});
    return { a_b: true };
  });
  expect(select).toHaveBeenLastCalledWith("series:alpha");
  adapter.onCellSelectionChange(() => []);
  expect(select).toHaveBeenLastCalledWith(null);
  expect(() =>
    adapter.onCellSelectionChange([
      {
        anchorRowId: "a",
        focusRowId: "a_b",
        anchorColumnId: "value",
        focusColumnId: "value",
      },
    ]),
  ).toThrow("ranges");
  expect(
    tableSelection(rows.slice(1), getRowId, ["value"], {
      ...props,
      getActiveCell,
    }).state.cellSelection,
  ).toEqual([]);
  expect(
    tableSelection(rows, getRowId, ["series"], { ...props, getActiveCell })
      .state.cellSelection,
  ).toEqual([]);
  expect(
    tableSelection(rows, getRowId, ["value"], {
      ...props,
      getActiveCell: () => ({ rowId: "a", columnId: "value" }),
    }).state.cellSelection,
  ).toEqual([]);
});
const callbacks = {
  row: vi.fn(),
  cell: vi.fn(),
  proposal: vi.fn(),
  action: vi.fn(),
};
function Example() {
  const [key, setKey] = useState<string | null>(null),
    [reject, setReject] = useState(false),
    [reverse, setReverse] = useState(false),
    [remove, setRemove] = useState(false);
  const link = useLinkedSelection({
    selectedKey: key,
    onSelectedKeyChange(next) {
      callbacks.proposal(next);
      if (!reject) setKey(next);
    },
  });
  const data = rows.filter((row) => !remove || row.id !== "a_b");
  if (reverse) data.reverse();
  return (
    <section aria-label="table instance">
      <button onClick={() => link.select("point:alpha")}>External</button>
      <button onClick={link.clear}>Clear</button>
      <button onClick={() => setKey("absent")}>Unmapped</button>
      <button onClick={() => setReject(!reject)}>Reject</button>
      <button onClick={() => setReverse(!reverse)}>Reverse</button>
      <button onClick={() => setRemove(!remove)}>Remove</button>
      <output>{key ?? "none"}</output>
      <FinstackTable<Row, bigint>
        caption="Stored rows"
        data={data}
        getRowId={getRowId}
        columns={[
          { id: "series", accessorKey: "series", header: "Series" },
          {
            id: "value",
            accessorKey: "value",
            header: "Value",
            cell: ({ getValue }) => String(getValue()),
          },
          {
            id: "action",
            header: "Action",
            cell: () => (
              <>
                <button onClick={callbacks.action}>Inspect</button>
                <input aria-label="Row note" defaultValue="original" />
              </>
            ),
          },
        ]}
        link={link}
        getRowKey={getRowKey}
        getCellKey={getCellKey}
        getActiveCell={getActiveCell}
        onRowActivate={callbacks.row}
        onCellActivate={(value: CellActivation<Row, bigint>) =>
          callbacks.cell(value)
        }
        density="compact"
        totals={{ value: "Supplied total: 18014398509481988" }}
      />
    </section>
  );
}
it("composes gestures once, preserves typed values and protects nested controls", async () => {
  Object.values(callbacks).forEach((fn) => fn.mockClear());
  const user = userEvent.setup();
  const { container } = render(<Example />);
  await user.click(screen.getByText("alpha"));
  expect(callbacks.proposal).toHaveBeenCalledExactlyOnceWith("series:alpha");
  expect(callbacks.row).toHaveBeenCalledExactlyOnceWith(rows[0]);
  const cell = screen.getByText("9007199254740993").closest("td")!;
  await user.click(cell);
  expect(callbacks.proposal).toHaveBeenCalledTimes(2);
  expect(callbacks.row).toHaveBeenCalledTimes(1);
  expect(callbacks.cell).toHaveBeenCalledExactlyOnceWith({
    row: rows[0],
    rowId: "a_b",
    columnId: "value",
    value: 9007199254740993n,
  });
  expect(cell.getAttribute("aria-selected")).toBe("true");
  await user.click(screen.getAllByRole("button", { name: "Inspect" })[0]);
  expect(callbacks.action).toHaveBeenCalledTimes(1);
  expect(callbacks.proposal).toHaveBeenCalledTimes(2);
  const editor = screen.getAllByRole("textbox", { name: "Row note" })[0];
  await user.clear(editor);
  await user.type(editor, "changed");
  expect(callbacks.proposal).toHaveBeenCalledTimes(2);
  cell.focus();
  await user.keyboard("{Enter}");
  expect(callbacks.cell).toHaveBeenCalledTimes(2);
  expect(callbacks.proposal).toHaveBeenCalledTimes(2);
  screen.getByText("beta").closest("tr")!.focus();
  await user.keyboard(" ");
  expect(callbacks.proposal).toHaveBeenLastCalledWith("series:beta");
  expect(callbacks.row).toHaveBeenCalledTimes(2);
  expect((await axe(container)).violations).toEqual([]);
});
it("rejects proposals without stale highlights, replayed activation or focus transfer", async () => {
  Object.values(callbacks).forEach((fn) => fn.mockClear());
  const user = userEvent.setup();
  render(<Example />);
  await user.click(screen.getByRole("button", { name: "External" }));
  await user.click(screen.getByRole("button", { name: "Reject" }));
  const alpha = screen.getByText("9007199254740993").closest("td")!,
    beta = screen.getByText("9007199254740995").closest("td")!;
  await user.click(beta);
  expect(alpha.getAttribute("aria-selected")).toBe("true");
  expect(beta.getAttribute("aria-selected")).toBe("false");
  expect(callbacks.proposal).toHaveBeenCalledTimes(2);
  beta.focus();
  fireEvent.click(screen.getByRole("button", { name: "Reverse" }));
  expect(document.activeElement).toBe(beta);
  expect(alpha.getAttribute("aria-selected")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  expect(document.querySelector('td[aria-selected="true"]')).toBeNull();
  expect(callbacks.proposal).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "Unmapped" }));
  expect(document.activeElement).toBe(beta);
  expect(callbacks.cell).toHaveBeenCalledTimes(1);
});
it("separates instances and clears through the public hook", async () => {
  Object.values(callbacks).forEach((fn) => fn.mockClear());
  const user = userEvent.setup();
  render(
    <>
      <Example />
      <Example />
    </>,
  );
  const [first, second] = screen.getAllByRole("region", {
    name: "table instance",
  });
  await user.click(within(first).getByText("alpha"));
  expect(second.querySelector('[aria-selected="true"]')).toBeNull();
  await user.click(within(first).getByRole("button", { name: "Clear" }));
  expect(first.querySelector('[aria-selected="true"]')).toBeNull();
});
it("keeps many-to-one cell matches decorative until an explicit active address is supplied", () => {
  const link = { selectedKey: "shared", select: vi.fn(), clear: vi.fn() };
  const mapping = { link, getCellKey: () => "shared" };
  expect(
    tableSelection(rows, getRowId, ["value"], mapping).state.cellSelection,
  ).toEqual([]);
  expect(
    tableSelection(rows, getRowId, ["value"], {
      ...mapping,
      getActiveCell: () => ({ rowId: "a", columnId: "value" }),
    }).state.cellSelection,
  ).toHaveLength(1);
  expect(link.select).not.toHaveBeenCalled();
  expect(() =>
    tableSelection([...rows, rows[0]], getRowId, ["value"], mapping),
  ).toThrow("unique");
});
it("renders supplied totals and explicit empty/error/loading states without calculation", () => {
  const props = {
    data: [] as Row[],
    columns: [{ id: "value", accessorKey: "value", header: "Value" }],
    getRowId,
    caption: "Empty table",
    density: "comfortable" as const,
    totals: { value: "USD 7.0000000001" },
    emptyState: <span>Nothing loaded</span>,
  };
  const view = render(<FinstackTable {...props} />);
  expect(screen.getByText("USD 7.0000000001")).toBeTruthy();
  expect(screen.getByText("Nothing loaded")).toBeTruthy();
  expect(
    view.container.querySelector('[data-density="comfortable"]'),
  ).toBeTruthy();
  view.rerender(<FinstackTable {...props} loading />);
  expect(screen.getByRole("status").textContent).toBe("Loading…");
  view.rerender(<FinstackTable {...props} error="Native result unavailable" />);
  expect(screen.getByRole("alert").textContent).toBe(
    "Native result unavailable",
  );
});
