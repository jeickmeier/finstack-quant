import { formatRaw, formatSigned } from "@/lib/finstack/format/format";
/** Display one supplied measure. Optional units need their source; missing values never become zero. */
export function MeasureValue({
  value,
  unit,
  precision,
  signed = false,
  label,
}: {
  value: number | null | undefined;
  unit?: { label: string; source: string };
  precision?: number;
  signed?: boolean;
  label?: string;
}) {
  const raw = formatRaw(value);
  const text =
    value == null
      ? raw
      : precision === undefined
        ? raw
        : value.toFixed(precision);
  if (unit && !unit.source.trim())
    throw new Error("Measure units require a canonical source");
  return (
    <span
      aria-label={label}
      className="finstack-numeric font-sans text-base text-foreground"
      title={raw}
    >
      {signed && value != null ? formatSigned(text) : text}
      <span className="ml-1 text-xs text-muted-foreground">
        {unit?.label ?? "Unit unavailable"}
      </span>
    </span>
  );
}
