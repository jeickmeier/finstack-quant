"use client";
import { useState } from "react";
import { FieldFrame } from "@/components/finstack/primitives/field-frame/field-frame";
import { FinstackTable } from "@/components/finstack/primitives/finstack-table/finstack-table";
import { DecimalInput } from "@/components/finstack/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/primitives/enum-field/enum-field";
import { MoneyInput } from "@/components/finstack/primitives/money-input/money-input";
import { RateInput } from "@/components/finstack/primitives/rate-input/rate-input";
import {
  TenorInput,
  type TenorEdit,
} from "@/components/finstack/primitives/tenor-input/tenor-input";
import { DayCountSelect } from "@/components/finstack/primitives/day-count-select/day-count-select";
import { BdcSelect } from "@/components/finstack/primitives/bdc-select/bdc-select";
import { CalendarSelect } from "@/components/finstack/primitives/calendar-select/calendar-select";
import { ModelPicker } from "@/components/finstack/primitives/model-picker/model-picker";
import { MetricPicker } from "@/components/finstack/primitives/metric-picker/metric-picker";
import {
  KnotTable,
  type KnotEdit,
} from "@/components/finstack/primitives/knot-table/knot-table";
import { MeasureValue } from "@/components/finstack/primitives/measure-value/measure-value";
import { MoneyValue } from "@/components/finstack/primitives/money-value/money-value";
import { StampBadge } from "@/components/finstack/primitives/stamp-badge/stamp-badge";
import { JsonViewer } from "@/components/finstack/primitives/json-viewer/json-viewer";
import { DateInput } from "@/components/finstack/primitives/date-input/date-input";
import { IdCombobox } from "@/components/finstack/primitives/id-combobox/id-combobox";
import { FinstackChart } from "@/components/finstack/primitives/finstack-chart/finstack-chart";
import { figureExample } from "@/components/finstack/components/figure-example/figure-example";
import { useAppForm } from "@/lib/finstack/form";
import data from "./data.json";
const options = [
  { value: "alpha", label: "Alpha", group: "Supplied" },
  { value: "beta", label: "Beta", group: "Supplied" },
];
function FormDemo() {
  const [submitted, setSubmitted] = useState("");
  const form = useAppForm({
    defaultValues: { amount: "1234.567890123456789" },
    onSubmit: ({ value }) => {
      setSubmitted(value.amount);
    },
  });
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <form.AppForm>
        <form.AppField name="amount">
          {(field) => <field.DecimalField label="Exact amount" />}
        </form.AppField>
        <form.SubmitButton label="Submit amount" />
        <form.ResetButton />
      </form.AppForm>
      <output aria-label="Submitted amount" className="sr-only">
        {submitted}
      </output>
    </form>
  );
}
export function PrimitiveDemo({
  name,
  density,
  width,
  publication,
}: {
  name: string;
  density: "compact" | "comfortable";
  width: number;
  publication: boolean;
}) {
  const [text, setText] = useState("12345678901234567890.1234"),
    [choice, setChoice] = useState("alpha"),
    [rate, setRate] = useState<string | number>("0.0525"),
    [date, setDate] = useState("2024-02-29"),
    [money, setMoney] = useState({
      amount: "1000000.123456789",
      currency: "USD",
    }),
    [tenor, setTenor] = useState<TenorEdit>({ count: 6, unit: "months" }),
    [dayCount, setDayCount] = useState("act_365f"),
    [bdc, setBdc] = useState("following"),
    [calendar, setCalendar] = useState("sifma"),
    [metrics, setMetrics] = useState<string[]>(["alpha"]),
    [knots, setKnots] = useState<KnotEdit[]>([
      [1, 0.99],
      [2, 0.95],
    ]),
    [activations, setActivations] = useState(0);
  switch (name) {
    case "finstack-theme":
      return (
        <div className="grid grid-cols-2 gap-4">
          <div className="border border-border bg-background p-4 text-foreground">
            Background and foreground
          </div>
          <div className="border border-border bg-card p-4 text-card-foreground">
            Card
          </div>
          <div className="bg-primary p-4 text-primary-foreground">Primary</div>
          <div className="bg-muted p-4 text-muted-foreground">Muted text</div>
          <p className="font-mono">0123456789 · −0.125</p>
          <p>IBM Plex Sans · Typography</p>
        </div>
      );
    case "field-frame":
      return (
        <FieldFrame
          label="Reference"
          help="Caller-supplied reference. Escape dismisses this help."
        >
          {(control) => (
            <input
              {...control}
              value={text}
              onChange={(event) => setText(event.target.value)}
              className="finstack-field border border-border bg-background px-2"
            />
          )}
        </FieldFrame>
      );
    case "decimal-input":
      return (
        <DecimalInput
          label="Exact decimal"
          value={text}
          onValueChange={setText}
        />
      );
    case "enum-field":
      return (
        <EnumField
          label="Supplied choice"
          value={choice}
          onValueChange={setChoice}
          options={options}
        />
      );
    case "money-input":
      return (
        <MoneyInput
          label="Notional"
          value={money}
          onValueChange={setMoney}
          currencies={["USD", "EUR"]}
        />
      );
    case "rate-input":
      return <RateInput label="Rate" value={rate} onValueChange={setRate} />;
    case "tenor-input":
      return (
        <TenorInput label="Tenor" value={tenor} onValueChange={setTenor} />
      );
    case "day-count-select":
      return (
        <DayCountSelect
          label="Day count"
          value={dayCount}
          onValueChange={setDayCount}
        />
      );
    case "bdc-select":
      return (
        <BdcSelect
          label="Business day convention"
          value={bdc}
          onValueChange={setBdc}
        />
      );
    case "calendar-select":
      return (
        <CalendarSelect
          label="Calendar"
          value={calendar}
          onValueChange={setCalendar}
          options={[
            { value: "sifma", label: "SIFMA" },
            { value: "nyse", label: "NYSE" },
          ]}
        />
      );
    case "model-picker":
      return (
        <ModelPicker
          label="Supplied model"
          value={choice}
          onValueChange={setChoice}
          options={options}
        />
      );
    case "metric-picker":
      return (
        <MetricPicker
          label="Supplied metrics"
          value={metrics}
          onValueChange={setMetrics}
          options={options}
        />
      );
    case "knot-table":
      return (
        <KnotTable value={knots} rowIds={["a", "b"]} onValueChange={setKnots} />
      );
    case "measure-value":
      return (
        <MeasureValue
          label="Original measure"
          value={-0.0000123456789}
          signed
        />
      );
    case "money-value":
      return <MoneyValue value={money} />;
    case "stamp-badge":
      return <StampBadge meta={data.bond.result.meta} />;
    case "json-viewer":
      return (
        <JsonViewer
          label="Original JSON"
          text={'{"seed":18446744073709551615,"amount":"1000000.123456789"}'}
          density={density}
        />
      );
    case "date-input":
      return <DateInput label="As of" value={date} onValueChange={setDate} />;
    case "id-combobox":
      return (
        <IdCombobox
          label="Identifier"
          value={choice}
          onValueChange={setChoice}
          options={["alpha", "beta", "literal/id.with.dot"]}
        />
      );
    case "finstack-form":
      return <FormDemo />;
    case "finstack-table":
      return (
        <>
          <FinstackTable
            caption="Supplied exact values"
            density={density}
            data={[
              { id: "USD", value: "1000000.123456789" },
              { id: "EUR", value: "999.99" },
            ]}
            columns={[
              { id: "value", header: "Value", accessorKey: "value" },
              {
                id: "action",
                header: "Details",
                cell: () => (
                  <button onClick={() => setActivations((n) => n + 1)}>
                    Open detail
                  </button>
                ),
              },
            ]}
            getRowId={(row) => row.id}
            onRowActivate={() => setActivations((n) => n + 1)}
          />
          <output aria-label="Activation count" className="sr-only">
            {activations}
          </output>
        </>
      );
    case "finstack-chart":
      return (
        <>
          <FinstackChart
            {...figureExample}
            width={width}
            height={publication ? 760 : 560}
          />
        </>
      );
    default:
      throw new Error(`Missing primitive harness: ${name}`);
  }
}
