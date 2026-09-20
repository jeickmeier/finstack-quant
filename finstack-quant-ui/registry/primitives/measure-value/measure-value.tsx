import { formatRaw, formatSigned } from "@/lib/finstack/format/format";
/** Display one supplied measure. Optional units need their source; missing values never become zero. */
export function MeasureValue({
  value,
  unit,
  precision,
  signed = false,
  label,
  showUnavailableUnit = true,
}: {
  value: number | null | undefined;
  unit?: { label: string; source: string };
  precision?: number;
  signed?: boolean;
  label?: string;
  /** Hide only the missing-unit note when the enclosing table states it once. */
  showUnavailableUnit?: boolean;
}) {
  const raw = formatRaw(value);
  const exact =
    value == null
      ? raw
      : precision === undefined
        ? raw
        : value.toFixed(precision);
  // Display-only digit grouping of plain decimals; every digit of the raw value is retained.
  const text = /^-?\d{4,}(\.\d+)?$/.test(exact)
    ? exact.replace(
        /^(-?)(\d+)/,
        (_, sign, digits) =>
          sign + digits.replace(/\B(?=(\d{3})+(?!\d))/g, ","),
      )
    : exact;
  if (unit && !unit.source.trim())
    throw new Error("Measure units require a canonical source");
  return (
    <span
      aria-label={label}
      className="finstack-numeric font-sans text-foreground"
      title={raw}
    >
      {signed && value != null ? formatSigned(text) : text}
      {(unit || showUnavailableUnit) && (
        <span className="finstack-na ml-1 text-xs whitespace-nowrap">
          {unit?.label ?? "Unit unavailable"}
        </span>
      )}
    </span>
  );
}
