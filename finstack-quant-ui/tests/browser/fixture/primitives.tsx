import { useState } from "react";
import { createRoot } from "react-dom/client";
import { DecimalInput } from "./components/finstack/primitives/decimal-input/decimal-input";
import { MoneyInput } from "./components/finstack/primitives/money-input/money-input";
import { RateInput } from "./components/finstack/primitives/rate-input/rate-input";
import { DateInput } from "./components/finstack/primitives/date-input/date-input";
import { IdCombobox } from "./components/finstack/primitives/id-combobox/id-combobox";
import { ModelPicker } from "./components/finstack/primitives/model-picker/model-picker";
import { MetricPicker } from "./components/finstack/primitives/metric-picker/metric-picker";
import {
  KnotTable,
  type KnotEdit,
} from "./components/finstack/primitives/knot-table/knot-table";
import { JsonViewer } from "./components/finstack/primitives/json-viewer/json-viewer";
import { FinstackTable } from "./components/finstack/primitives/finstack-table/finstack-table";
import { MoneyValue } from "./components/finstack/primitives/money-value/money-value";
import data from "./data.json";
const options = Array.from(
  { length: 10000 },
  (_, i) => `ID-${String(i).padStart(5, "0")}`,
);
function App() {
  const [decimal, setDecimal] = useState("12345678901234567890.1234"),
    [money, setMoney] = useState(data.notional),
    [rate, setRate] = useState<string | number>("0.025"),
    [date, setDate] = useState("2024-02-29"),
    [id, setId] = useState(""),
    [model, setModel] = useState("a"),
    [metrics, setMetrics] = useState<string[]>([]),
    [knots, setKnots] = useState<KnotEdit[]>([
      [1, 0.99],
      [2, 0.95],
    ]);
  return (
    <main className="mx-auto max-w-5xl space-y-6 p-6">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-medium">Registry primitives</h1>
          <p className="text-sm text-muted-foreground">
            Controlled inputs · exact values · shared presentation
          </p>
        </div>
        <div className="flex gap-2">
          <button
            className="rounded-sm border border-border px-2"
            onClick={() =>
              (document.documentElement.dataset.theme =
                document.documentElement.dataset.theme === "dark"
                  ? "light"
                  : "dark")
            }
          >
            Theme
          </button>
          <button
            className="rounded-sm border border-border px-2"
            onClick={() =>
              (document.documentElement.dataset.density =
                document.documentElement.dataset.density === "comfortable"
                  ? "compact"
                  : "comfortable")
            }
          >
            Density
          </button>
        </div>
      </header>
      <section
        aria-label="Controlled inputs"
        className="grid gap-4 rounded-md border border-border bg-card p-4 sm:grid-cols-2"
      >
        <DecimalInput
          label="Exact decimal"
          help="Canonical decimal strings retain their supplied precision."
          value={decimal}
          onValueChange={setDecimal}
        />
        <RateInput label="Rate" value={rate} onValueChange={setRate} />
        <MoneyInput
          label="Notional"
          value={money}
          onValueChange={setMoney}
          currencies={["USD", "EUR"]}
        />
        <DateInput label="As of" value={date} onValueChange={setDate} />
        <IdCombobox
          label="Identifier"
          options={options}
          value={id}
          onValueChange={setId}
        />
        <ModelPicker
          label="Supplied model"
          value={model}
          onValueChange={setModel}
          options={[
            { value: "a", label: "Alpha", group: "Available" },
            { value: "b", label: "Beta", group: "Available" },
          ]}
        />
        <MetricPicker
          label="Supplied metrics"
          value={metrics}
          onValueChange={setMetrics}
          options={[
            { value: "one", label: "First", group: "Available" },
            { value: "two", label: "Second", group: "Available" },
          ]}
        />
        <div>
          <p className="text-xs text-muted-foreground">Supplied notional</p>
          <MoneyValue value={money} />
        </div>
      </section>
      <section className="rounded-md border border-border bg-card p-4">
        <h2 className="mb-2 text-lg">Editable stored knots</h2>
        <KnotTable
          value={knots}
          rowIds={["a", "b"]}
          onValueChange={setKnots}
          xLabel="Coordinate"
          yLabel="Value"
        />
      </section>
      <section className="rounded-md border border-border bg-card p-4">
        <JsonViewer label="Original supplied JSON" text={data.text} />
      </section>
      <section
        className="w-80 max-w-full"
        aria-label="Wide read-only table regression"
      >
        <h2 id="wide-table-start" tabIndex={-1}>
          Wide read-only values
        </h2>
        <FinstackTable
          caption="Wide supplied values"
          data={[
            {
              id: "row-1",
              first: "12345678901234567890.1234",
              second: "98765432109876543210.4321",
              third: "18446744073709551615",
            },
          ]}
          columns={[
            {
              id: "first",
              accessorKey: "first",
              header: "First supplied coordinate",
            },
            {
              id: "second",
              accessorKey: "second",
              header: "Second supplied coordinate",
            },
            {
              id: "third",
              accessorKey: "third",
              header: "Third supplied coordinate",
            },
          ]}
          getRowId={(row) => row.id}
        />
      </section>
      <output aria-label="Accepted ID" className="font-mono text-sm">
        {id}
      </output>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
