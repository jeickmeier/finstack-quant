"use client";
import { EnumField, type EnumFieldProps } from "../enum-field/enum-field";
/** Calendar options are supplied by the host; this control has no local holiday registry. */
export function CalendarSelect(props: EnumFieldProps) {
  return <EnumField {...props} layout="select" />;
}
