"use client";
import type { ExampleProps } from "./examples/props";
import { Example as Example0 } from "./examples/finstack-theme";
import { Example as Example1 } from "./examples/field-frame";
import { Example as Example2 } from "./examples/decimal-input";
import { Example as Example3 } from "./examples/enum-field";
import { Example as Example4 } from "./examples/money-input";
import { Example as Example5 } from "./examples/rate-input";
import { Example as Example6 } from "./examples/tenor-input";
import { Example as Example7 } from "./examples/day-count-select";
import { Example as Example8 } from "./examples/bdc-select";
import { Example as Example9 } from "./examples/metric-picker";
import { Example as Example10 } from "./examples/knot-table";
import { Example as Example11 } from "./examples/measure-value";
import { Example as Example12 } from "./examples/money-value";
import { Example as Example13 } from "./examples/stamp-badge";
import { Example as Example14 } from "./examples/json-viewer";
import { Example as Example15 } from "./examples/date-input";
import { Example as Example16 } from "./examples/id-combobox";
import { Example as Example17 } from "./examples/finstack-form";
import { Example as Example18 } from "./examples/finstack-table";
import { Example as Example19 } from "./examples/finstack-chart";

const examples = {
  "finstack-theme": Example0,
  "field-frame": Example1,
  "decimal-input": Example2,
  "enum-field": Example3,
  "money-input": Example4,
  "rate-input": Example5,
  "tenor-input": Example6,
  "day-count-select": Example7,
  "bdc-select": Example8,
  "metric-picker": Example9,
  "knot-table": Example10,
  "measure-value": Example11,
  "money-value": Example12,
  "stamp-badge": Example13,
  "json-viewer": Example14,
  "date-input": Example15,
  "id-combobox": Example16,
  "finstack-form": Example17,
  "finstack-table": Example18,
  "finstack-chart": Example19,
};
export function PrimitiveDemo({
  name,
  ...props
}: ExampleProps & { name: string }) {
  const Example = examples[name as keyof typeof examples];
  if (!Example) throw new Error(`Missing primitives example: ${name}`);
  return <Example {...props} />;
}
