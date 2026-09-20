"use client";
import { useState } from "react";
import { Popover } from "@base-ui/react/popover";
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
  const [query, setQuery] = useState("");
  const groups = Map.groupBy(
    options.filter((option) =>
      `${option.label} ${option.group ?? ""}`
        .toLowerCase()
        .includes(query.toLowerCase()),
    ),
    (option) => option.group ?? "",
  );
  return (
    <FieldFrame {...field}>
      {(control) => (
        <Popover.Root>
          <Popover.Trigger
            {...control}
            disabled={disabled}
            className="finstack-field flex w-full items-center justify-between gap-2 rounded-sm border border-control-border bg-background px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring"
          >
            <span>
              {value.length ? `${value.length} selected` : "Select metrics"}
            </span>
            <span aria-hidden="true">⌄</span>
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Positioner sideOffset={4}>
              <Popover.Popup className="w-80 max-w-[calc(100vw-2rem)] rounded-md border border-border bg-card p-3 text-card-foreground shadow-[var(--elevation)]">
                <Popover.Title className="mb-2 text-sm font-medium">
                  {field.label}
                </Popover.Title>
                <input
                  aria-label="Search metrics"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Search metrics…"
                  className="finstack-field mb-2 w-full rounded-sm border border-control-border bg-background px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring"
                />
                <div
                  role="group"
                  aria-label={field.label}
                  className="max-h-64 space-y-3 overflow-auto"
                >
                  {!groups.size && (
                    <p className="text-sm text-muted-foreground">
                      No matching metrics
                    </p>
                  )}
                  {[...groups].map(([group, items]) => (
                    <fieldset key={group} disabled={disabled}>
                      <legend className="text-xs text-muted-foreground">
                        {group}
                      </legend>
                      {items.map((option) => (
                        <label
                          key={option.value}
                          title={option.description}
                          className="finstack-row flex items-center gap-2 border-b border-border text-sm"
                        >
                          <Checkbox.Root
                            checked={value.includes(option.value)}
                            disabled={disabled || option.disabled}
                            onCheckedChange={(checked) =>
                              onValueChange(
                                checked
                                  ? [...new Set([...value, option.value])]
                                  : value.filter(
                                      (item) => item !== option.value,
                                    ),
                              )
                            }
                            className="size-4 shrink-0 rounded-sm border border-control-border data-checked:bg-primary data-checked:text-primary-foreground focus-visible:outline-2 focus-visible:outline-ring"
                          >
                            <Checkbox.Indicator>✓</Checkbox.Indicator>
                          </Checkbox.Root>
                          {option.label}
                        </label>
                      ))}
                    </fieldset>
                  ))}
                </div>
              </Popover.Popup>
            </Popover.Positioner>
          </Popover.Portal>
        </Popover.Root>
      )}
    </FieldFrame>
  );
}
