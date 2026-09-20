import type { MoneyValue as Money } from "finstack-quant-wasm";
import { formatMoney, type RoundingStamp } from "@/lib/finstack/format/format";
/** Currency-tagged exact supplied money, using its returned rounding stamp for display only. */
export function MoneyValue({
  value,
  rounding,
  label,
}: {
  value: Money | null | undefined;
  rounding?: RoundingStamp;
  label?: string;
}) {
  return (
    <span
      aria-label={label}
      className="finstack-numeric font-sans text-base text-foreground"
      title={value?.amount}
    >
      {value ? formatMoney(value, rounding) : "—"}
    </span>
  );
}
