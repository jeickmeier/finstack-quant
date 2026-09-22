import type { MoneyValue as Money } from "finstack-quant-wasm";
import { formatRawMoney } from "@/lib/finstack/format/format";
/** Currency-tagged exact supplied money; displays caller-prepared text or the raw exact amount. No WASM initialization or provider is required. */
export function MoneyValue({
  value,
  displayText,
  label,
  prominent = false,
}: {
  value: Money | null | undefined;
  /** Prepared display text for this exact money/currency (e.g. worker-formatted); the raw exact amount is shown when omitted. */
  displayText?: string;
  label?: string;
  /** Emphasize the amount while keeping its supplied currency visible. */
  prominent?: boolean;
}) {
  const text = value ? (displayText ?? formatRawMoney(value)) : "—";
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
