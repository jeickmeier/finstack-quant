"use client";
import { EnumField, type EnumFieldProps } from "../enum-field/enum-field";
/** Select a supplied pricing model; availability and compatibility remain caller-owned. */
export function ModelPicker(props: EnumFieldProps) {
  return <EnumField {...props} layout="select" />;
}
