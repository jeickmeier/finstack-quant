import type { MoneyValue as Money } from "finstack-quant-wasm";
import { formatMoney, type RoundingStamp } from "@/lib/finstack/format/format";
/** Currency-tagged exact supplied money, using its returned rounding stamp for display only. */
export function MoneyValue({
  value,
  rounding,
  label,
  prominent = false,
}: {
  value: Money | null | undefined;
  rounding?: RoundingStamp;
  label?: string;
  /** Emphasize the amount while keeping its supplied currency visible. */
  prominent?: boolean;
}) {
  const text = value ? formatMoney(value, rounding) : "—";
  return (
    <span
      aria-label={label}
      className={
        prominent
          ? "finstack-money finstack-money--prominent finstack-numeric"
          : "finstack-numeric font-sans text-foreground"
      }
      title={value?.amount}
    >
      {prominent && value ? (
        <>
          <span className="finstack-money__currency">{value.currency}</span>{" "}
          <span className="finstack-money__amount">
            {text.slice(value.currency.length + 1)}
          </span>
        </>
      ) : (
        text
      )}
    </span>
  );
}
