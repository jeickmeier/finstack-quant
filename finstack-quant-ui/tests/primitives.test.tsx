// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  render,
  screen,
  cleanup,
  fireEvent,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { DecimalInput } from "../registry/primitives/decimal-input/decimal-input";
import { MoneyInput } from "../registry/primitives/money-input/money-input";
import { RateInput } from "../registry/primitives/rate-input/rate-input";
import { TenorInput } from "../registry/primitives/tenor-input/tenor-input";
import { EnumField } from "../registry/primitives/enum-field/enum-field";
import { DayCountSelect } from "../registry/primitives/day-count-select/day-count-select";
import { BdcSelect } from "../registry/primitives/bdc-select/bdc-select";
import { MetricPicker } from "../registry/primitives/metric-picker/metric-picker";
import { IdCombobox } from "../registry/primitives/id-combobox/id-combobox";
import { DateInput } from "../registry/primitives/date-input/date-input";
import { FinstackTable } from "../registry/primitives/finstack-table/finstack-table";
import { KnotTable } from "../registry/primitives/knot-table/knot-table";
import { MoneyValue } from "../registry/primitives/money-value/money-value";
import { MeasureValue } from "../registry/primitives/measure-value/measure-value";
import { StampBadge } from "../registry/primitives/stamp-badge/stamp-badge";
import { JsonViewer } from "../registry/primitives/json-viewer/json-viewer";
import contracts from "../src/generated/primitive-contracts.json";
import { idColumn, moneyColumn } from "../src/format/columns";

beforeAll(() => {
  document.documentElement.style.setProperty("--row-height", "28px");
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
  HTMLElement.prototype.scrollIntoView = vi.fn();
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
const options = [
  { value: "a", label: "Alpha", description: "First choice" },
  { value: "b", label: "Beta", description: "Second choice" },
];
const presentation = {
  source: "core.Rate",
  wire: "decimal",
  display: "percent",
} as const;

it("keeps exact decimal and incomplete editing state controlled, grouping only on blur", async () => {
  const changes: string[] = [];
  function Example() {
    const [value, setValue] = useState("12345678901234567890.1234");
    return (
      <DecimalInput
        label="Amount"
        value={value}
        onValueChange={(value) => {
          changes.push(value);
          setValue(value);
        }}
      />
    );
  }
  render(<Example />);
  const input = screen.getByRole("textbox") as HTMLInputElement;
  expect(input.value).toBe("12,345,678,901,234,567,890.1234");
  fireEvent.focus(input);
  expect(input.value).toBe("12345678901234567890.1234");
  fireEvent.change(input, { target: { value: "-" } });
  expect(input.value).toBe("-");
  expect(changes.at(-1)).toBe("-");
  fireEvent.change(input, { target: { value: "1.23e4" } });
  fireEvent.blur(input);
  expect(input.value).toBe("12,300");
  expect(changes.at(-1)).toBe("1.23e4");
});
it("passes supplied decimal constraints and never filters invalid edits", () => {
  const change = vi.fn();
  render(
    <DecimalInput
      label="Amount"
      value="abc"
      onValueChange={change}
      pattern="^special$"
      min="0.01"
      max="12"
      scale={3}
      allowNegative={false}
    />,
  );
  const input = screen.getByRole("textbox");
  expect(input.getAttribute("pattern")).toBe("^special$");
  expect(input.getAttribute("min")).toBe("0.01");
  expect(input.getAttribute("max")).toBe("12");
  expect(input.getAttribute("aria-invalid")).toBe("true");
  fireEvent.change(input, { target: { value: "" } });
  expect(change).toHaveBeenCalledWith("");
});
it("edits money without changing amount precision when currency changes", async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(
    <MoneyInput
      label="Notional"
      value={{ amount: "12345678901234567890.1", currency: "USD" }}
      currencies={["USD", "EUR"]}
      onValueChange={change}
    />,
  );
  screen.getByRole("combobox").focus();
  await user.keyboard("{ArrowDown}");
  await user.click(await screen.findByRole("option", { name: "EUR" }));
  expect(change).toHaveBeenLastCalledWith({
    amount: "12345678901234567890.1",
    currency: "EUR",
  });
});
it("converts only explicit rate units and permits invalid/empty working text", () => {
  const change = vi.fn();
  const { rerender } = render(
    <RateInput
      label="Rate"
      value="0.12345678901234567890"
      onValueChange={change}
      presentation={presentation}
    />,
  );
  const input = screen.getByRole("textbox") as HTMLInputElement;
  fireEvent.focus(input);
  expect(input.value).toBe("12.345678901234567890");
  fireEvent.change(input, { target: { value: "12.345678901234567891" } });
  expect(change).toHaveBeenLastCalledWith("0.12345678901234567891");
  rerender(
    <RateInput
      label="Rate"
      value="0.12345678901234567890"
      onValueChange={change}
    />,
  );
  expect(input.value).toBe("0.12345678901234567890");
  expect(screen.getByText("Unit unavailable")).toBeTruthy();
  fireEvent.change(input, { target: { value: "" } });
  expect(change).toHaveBeenLastCalledWith("");
});
it("keeps explicit number-representation rates numeric after valid editing", () => {
  const change = vi.fn();
  render(
    <RateInput
      label="Rate"
      value={0.1}
      representation="number"
      onValueChange={change}
    />,
  );
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "0.2" } });
  expect(change).toHaveBeenCalledWith(0.2);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "0x10" } });
  expect(change).toHaveBeenLastCalledWith("0x10");
});
it("edits tenor count without inventing a schema minimum", () => {
  const change = vi.fn();
  render(
    <TenorInput
      label="Tenor"
      value={{ count: 3, unit: "months" }}
      onValueChange={change}
    />,
  );
  const input = screen.getByRole("textbox");
  expect(input.getAttribute("aria-invalid")).toBe("false");
  expect(input.getAttribute("min")).toBe(
    String(contracts.tenor.schema.properties.count.minimum),
  );
  fireEvent.change(input, { target: { value: "0" } });
  expect(change).toHaveBeenCalledWith({ count: 0, unit: "months" });
  fireEvent.change(input, { target: { value: "" } });
  expect(change).toHaveBeenLastCalledWith({ count: "", unit: "months" });
  fireEvent.change(input, { target: { value: "0x10" } });
  expect(change).toHaveBeenLastCalledWith({ count: "0x10", unit: "months" });
});
it("supports keyboard radio changes and supplied help with Escape focus restoration", async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(
    <EnumField
      label="Choice"
      help="Canonical description"
      options={options}
      value="a"
      onValueChange={change}
    />,
  );
  await user.click(screen.getByRole("radio", { name: "Alpha" }));
  await user.keyboard("{ArrowRight}");
  expect(change).toHaveBeenCalledWith("b");
  await user.click(screen.getByRole("button", { name: "Choice help" }));
  expect(screen.getByText("Canonical description")).toBeTruthy();
  await user.keyboard("{Escape}");
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Choice help" }),
    ),
  );
});
it.each([DayCountSelect, BdcSelect])(
  "select primitive %# works with supplied options",
  async (Component) => {
    const user = userEvent.setup();
    const change = vi.fn();
    render(
      <Component
        label="Choice"
        value="a"
        options={options}
        onValueChange={change}
      />,
    );
    await user.click(screen.getByRole("combobox"));
    await waitFor(() =>
      expect(screen.getByRole("listbox").contains(document.activeElement)).toBe(
        true,
      ),
    );
    await user.keyboard("{End}");
    await waitFor(() =>
      expect(
        screen
          .getByRole("option", { name: /Beta/ })
          .hasAttribute("data-highlighted"),
      ).toBe(true),
    );
    await user.keyboard("{Enter}");
    expect(change).toHaveBeenCalledWith("b");
  },
);
it("changes grouped metrics while preserving supplied selections outside the shown list", async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(
    <MetricPicker
      label="Metrics"
      value={["hidden"]}
      options={options.map((option) => ({ ...option, group: "Risk" }))}
      onValueChange={change}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Metrics" }));
  await user.click(screen.getByRole("checkbox", { name: "Alpha" }));
  expect(change).toHaveBeenCalledWith(["hidden", "a"]);
});
it("accepts free-text IDs without requiring a suggestion", async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  function Example() {
    const [value, setValue] = useState("");
    return (
      <IdCombobox
        label="Curve ID"
        value={value}
        options={["USD-OIS"]}
        onValueChange={(value) => {
          setValue(value);
          change(value);
        }}
      />
    );
  }
  render(<Example />);
  await user.type(screen.getByRole("combobox"), "NEW-CURVE");
  expect(change).toHaveBeenLastCalledWith("NEW-CURVE");
});
it("date text preserves invalid edits and calendar selection emits ISO", async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(
    <DateInput
      label="As of"
      value="2024-02-29"
      onValueChange={change}
      min="2024-02-01"
      max="2024-03-01"
    />,
  );
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "2024-02-" },
  });
  expect(change).toHaveBeenCalledWith("2024-02-");
  await user.click(screen.getByRole("button", { name: "As of calendar" }));
  await user.click(screen.getByRole("button", { name: /February 28/ }));
  expect(change).toHaveBeenLastCalledWith("2024-02-28");
});
it("shared table uses stable IDs and per-row currencies with working empty/error states", () => {
  type Row = { id: string; money: { amount: string; currency: string } };
  const data: Row[] = [
    { id: "a", money: { amount: "1.23", currency: "USD" } },
    { id: "b", money: { amount: "2.34", currency: "EUR" } },
  ];
  const columns = [
    idColumn("ID", (r: Row) => r.id),
    moneyColumn("Value", (r: Row) => r.money),
  ];
  const { rerender } = render(
    <FinstackTable
      caption="Values"
      data={data}
      columns={columns}
      getRowId={(r) => r.id}
    />,
  );
  expect(screen.getByText("USD 1.23")).toBeTruthy();
  expect(screen.getByText("EUR 2.34")).toBeTruthy();
  rerender(
    <FinstackTable
      caption="Values"
      data={[]}
      columns={columns}
      getRowId={(r) => r.id}
    />,
  );
  expect(screen.getByText("No rows supplied")).toBeTruthy();
  rerender(
    <FinstackTable
      caption="Values"
      data={[]}
      columns={columns}
      getRowId={(r) => r.id}
      error="Returned error"
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe("Returned error");
});
it("knot editor composes the shared table and changes only the edited supplied coordinate", () => {
  const change = vi.fn();
  render(
    <KnotTable
      value={[
        [1, 0.99],
        [2, 0.95],
      ]}
      rowIds={["a", "b"]}
      xLabel="Term"
      yLabel="DF"
      onValueChange={change}
    />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "DF b" }), {
    target: { value: "0.94" },
  });
  expect(change).toHaveBeenCalledWith([
    [1, 0.99],
    [2, 0.94],
  ]);
  expect(screen.getByRole("table")).toBeTruthy();
});
it("value primitives show supplied values and explicit unavailable metadata", () => {
  render(
    <>
      <MeasureValue value={0.025} />
      <MoneyValue value={{ amount: "1.23", currency: "USD" }} />
      <StampBadge meta={null} />
    </>,
  );
  expect(screen.getByText("0.025")).toBeTruthy();
  expect(screen.getByText("Unit unavailable")).toBeTruthy();
  expect(screen.getByText("USD 1.23")).toBeTruthy();
  expect(screen.getByText("Metadata unavailable")).toBeTruthy();
});
it("shows caller-prepared money display text while the title keeps the raw source amount", () => {
  render(
    <MoneyValue
      value={{ amount: "1.245", currency: "USD" }}
      displayText="USD 1.24"
    />,
  );
  const node = screen.getByTitle("1.245");
  expect(node.textContent).toBe("USD 1.24");
});
it("formats JSON without losing numeric tokens and copies or downloads only the original bytes", async () => {
  const user = userEvent.setup();
  const write = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
  const print = vi.spyOn(window, "print").mockImplementation(() => {});
  const text =
    ' {"seed":18446744073709551615,"exponent":1.2300e+400,"decimal":1.2300,"amount":" 1.2300 ","message":" two  spaces "}\n';
  render(<JsonViewer text={text} downloadName="original.json" />);
  const pre = document.querySelector("pre")!;
  const printed = document.querySelector("code[data-json-print-source]")!;
  expect(printed.textContent).toBe(text);
  expect(printed.classList.contains("hidden")).toBe(true);
  expect(printed.classList.contains("print:block")).toBe(true);
  expect(pre.classList.contains("print:hidden")).toBe(true);
  expect(pre.textContent).toBe(
    '{\n  "seed": 18446744073709551615,\n  "exponent": 1.2300e+400,\n  "decimal": 1.2300,\n  "amount": " 1.2300 ",\n  "message": " two  spaces "\n}',
  );
  expect(
    screen
      .getByRole("button", { name: "Formatted" })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  await user.click(screen.getByRole("button", { name: "Original" }));
  expect(pre.textContent).toBe(text);
  expect(document.querySelector("code[data-json-print-source]")).toBe(printed);
  expect(printed.textContent).toBe(text);
  await user.click(screen.getByRole("button", { name: "Formatted" }));
  await user.click(screen.getByRole("button", { name: "Copy" }));
  expect(write).toHaveBeenCalledWith(text);
  await user.click(screen.getByRole("button", { name: "Print" }));
  expect(print).toHaveBeenCalledOnce();
  const originalURL = globalThis.URL;
  let downloaded: Blob | undefined;
  globalThis.URL = class extends originalURL {
    static createObjectURL(blob: Blob) {
      downloaded = blob;
      return "blob:original-json";
    }
    static revokeObjectURL() {}
  };
  const click = vi
    .spyOn(HTMLAnchorElement.prototype, "click")
    .mockImplementation(() => {});
  try {
    await user.click(screen.getByRole("button", { name: "Download" }));
    expect(click).toHaveBeenCalledOnce();
    const bytes = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = reject;
      reader.readAsText(downloaded!);
    });
    expect(bytes).toBe(text);
    await new Promise((resolve) => setTimeout(resolve, 1050));
  } finally {
    globalThis.URL = originalURL;
  }
});
it.each(["not JSON", '{"same":1,"same":2}'])(
  "keeps invalid or duplicate-key input in the original view: %s",
  (text) => {
    render(<JsonViewer text={text} />);
    expect(document.querySelector("pre")?.textContent).toBe(text);
    expect(
      (screen.getByRole("button", { name: "Formatted" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      screen
        .getByRole("button", { name: "Original" })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(
      screen.getByText(/Original text.*formatting unavailable/),
    ).toBeTruthy();
  },
);

it.each([
  <DecimalInput label="Decimal" value="1" onValueChange={() => {}} />,
  <MoneyInput
    label="Money"
    value={{ amount: "1.23", currency: "USD" }}
    currencies={["USD"]}
    onValueChange={() => {}}
  />,
  <RateInput label="Rate" value="0.1" onValueChange={() => {}} />,
  <TenorInput
    label="Tenor"
    value={{ count: 3, unit: "months" }}
    onValueChange={() => {}}
  />,
  <EnumField
    label="Enum"
    value="a"
    options={options}
    onValueChange={() => {}}
  />,
  <DayCountSelect label="Day count" value="act_360" onValueChange={() => {}} />,
  <BdcSelect label="BDC" value="following" onValueChange={() => {}} />,
  <EnumField
    layout="select"
    label="Calendar"
    value="a"
    options={options}
    onValueChange={() => {}}
  />,
  <EnumField
    layout="select"
    label="Model"
    value="a"
    options={options}
    onValueChange={() => {}}
  />,
  <MetricPicker
    label="Metrics"
    value={[]}
    options={options}
    onValueChange={() => {}}
  />,
  <IdCombobox label="ID" value="" options={[]} onValueChange={() => {}} />,
  <DateInput label="Date" value="2024-01-01" onValueChange={() => {}} />,
  <JsonViewer text="{}" />,
  <MeasureValue value={1} />,
  <MoneyValue value={null} />,
  <StampBadge meta={null} />,
  <FinstackTable
    caption="Empty table"
    data={[]}
    columns={[]}
    getRowId={() => "unused"}
  />,
  <KnotTable value={[[1, 2]]} rowIds={["a"]} onValueChange={() => {}} />,
])("primitive %# has no DOM accessibility violations", async (element) => {
  const { container } = render(element);
  const result = await axe(container, {
    rules: { "color-contrast": { enabled: false } },
  });
  expect(result.violations).toEqual([]);
});

it("JSON viewer supports undefined and error states without claiming a copy", () => {
  const { rerender } = render(<JsonViewer text={undefined} />);
  expect(screen.getByText("No JSON supplied")).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "Copy" }).hasAttribute("disabled"),
  ).toBe(true);
  rerender(<JsonViewer text={null} error="Native request failed" />);
  expect(screen.getByRole("alert").textContent).toBe("Native request failed");
});

it.each([
  [1000, undefined, "+1,000"],
  [-1234.5, undefined, "-1,234.5"],
  [0, undefined, "0"],
  [0.001, 0, "0"],
  [1234.567, 2, "+1,234.57"],
  [1e-7, undefined, "+1e-7"],
] as const)(
  "formats signed measure %s before grouping",
  (value, precision, text) => {
    render(
      <MeasureValue
        value={value}
        precision={precision}
        signed
        showUnavailableUnit={false}
      />,
    );
    expect(screen.getByText(text).getAttribute("title")).toBe(String(value));
  },
);
