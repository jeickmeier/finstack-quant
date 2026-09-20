"use client";
import { Checkbox } from "@base-ui/react/checkbox";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
import type { EnumOption } from "../enum-field/enum-field";
/** Controlled grouped metric selection. Hidden/unavailable supplied selections are not discarded. */
export function MetricPicker({
  value,
  onValueChange,
  options,
  disabled,
  ...field
}: FieldInfo & {
  value: string[];
  onValueChange: (value: string[]) => void;
  options: readonly EnumOption[];
  disabled?: boolean;
}) {
  const groups = Map.groupBy(options, (option) => option.group ?? "");
  return (
    <FieldFrame {...field}>
      {(control) => (
        <div {...control} role="group" className="space-y-2">
          {[...groups].map(([group, items]) => (
            <fieldset key={group} disabled={disabled}>
              <legend className="text-xs text-muted-foreground">{group}</legend>
              {items.map((option) => (
                <label
                  key={option.value}
                  title={option.description}
                  className="flex items-center gap-2 text-sm"
                >
                  <Checkbox.Root
                    checked={value.includes(option.value)}
                    disabled={disabled || option.disabled}
                    onCheckedChange={(checked) =>
                      onValueChange(
                        checked
                          ? [...new Set([...value, option.value])]
                          : value.filter((item) => item !== option.value),
                      )
                    }
                    className="size-4 rounded-sm border border-border data-checked:bg-primary data-checked:text-primary-foreground focus-visible:outline-2 focus-visible:outline-ring"
                  >
                    <Checkbox.Indicator>✓</Checkbox.Indicator>
                  </Checkbox.Root>
                  {option.label}
                </label>
              ))}
            </fieldset>
          ))}
        </div>
      )}
    </FieldFrame>
  );
}
