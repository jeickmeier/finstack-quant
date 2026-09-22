import {
  formatRaw,
  formatSigned,
  groupDecimal,
} from "@/lib/finstack/format/format";
/** Display one supplied measure. Optional units need their source; missing values never become zero. */
export function MeasureValue({
  value,
  unit,
  precision,
  displayText,
  signed = false,
  label,
  showUnavailableUnit = true,
}: {
  value: number | null | undefined;
  unit?: { label: string; source: string };
  precision?: number;
  /** Optional shorter presentation; the exact returned value remains in the title. */
  displayText?: string;
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
  const presented = displayText ?? exact;
  const signedText =
    signed && value != null ? formatSigned(presented) : presented;
  const text = /^[+-]?\d{4,}(\.\d+)?$/.test(signedText)
    ? signedText.startsWith("+")
      ? `+${groupDecimal(signedText.slice(1))}`
      : groupDecimal(signedText)
    : signedText;
  if (unit && !unit.source.trim())
    throw new Error("Measure units require a canonical source");
  return (
    <span
      aria-label={
        label ??
        (displayText && displayText !== raw
          ? `${text}; exact value ${raw}`
          : undefined)
      }
      className="finstack-numeric font-sans text-foreground"
      title={raw}
    >
      {displayText !== undefined && displayText !== raw ? (
        <>
          <span className="print:hidden">{text}</span>
          <span aria-hidden="true" className="hidden print:inline">
            {raw}
          </span>
        </>
      ) : (
        text
      )}
      {(unit || showUnavailableUnit) && (
        <span className="finstack-na ml-1 text-xs whitespace-nowrap">
          {unit?.label ?? "Unit unavailable"}
        </span>
      )}
    </span>
  );
}
