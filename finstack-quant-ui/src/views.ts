import curves from "./generated/curve-views.json";

/**
 * Project already-validated curve state using its canonical variant's declared fields.
 * @param state - A CurveState taken from validated market JSON; no WASM handles.
 * @returns Original stored knot coordinates, or declared fields for a no-knot variant.
 * @throws TypeError when the tag is unknown or the expected stored knots are absent.
 */
export function getCurveView(state: Record<string, unknown>) {
  const variant = curves.find((item) => item.type === state.type);
  if (!variant)
    throw new TypeError(`Unknown curve type: ${String(state.type)}`);
  if (variant.route === "stored-knots") {
    if (!Array.isArray(state.knot_points))
      throw new TypeError("Missing stored knot_points");
    return { kind: "knots" as const, points: state.knot_points, state };
  }
  return {
    kind: "fields" as const,
    fields: Object.fromEntries(
      variant.fields
        .filter((key) => Object.hasOwn(state, key))
        .map((key) => [key, state[key]]),
    ),
    state,
  };
}

/**
 * Preserve a value and source help without inferring a financial convention.
 * @param value - Supplied wire/host value, retained without numeric conversion.
 * @param metadata - Generated field metadata; only an explicit source x-unit is surfaced.
 * @returns Original value, source description and declared unit or an unavailable state.
 */
export function getValueView(
  value: unknown,
  metadata: { description?: string; "x-unit"?: unknown } = {},
) {
  return {
    value,
    description: metadata.description,
    unit: metadata["x-unit"] ?? null,
    unitStatus:
      metadata["x-unit"] == null
        ? ("unavailable" as const)
        : ("source" as const),
  };
}
