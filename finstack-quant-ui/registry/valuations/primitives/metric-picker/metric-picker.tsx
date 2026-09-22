"use client";
import { useState } from "react";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverTitle,
} from "@/components/ui/popover";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  FieldFrame,
  type FieldInfo,
} from "@/components/finstack/shared/primitives/field-frame/field-frame";
import type { EnumOption } from "@/components/finstack/shared/primitives/enum-field/enum-field";
/** Controlled metric selection using stock controls; unavailable selections remain unchanged. */
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
        <Popover>
          <PopoverTrigger
            {...control}
            disabled={disabled}
            render={<Button variant="outline" />}
          >
            {value.length ? `${value.length} selected` : "Select metrics"}
          </PopoverTrigger>
          <PopoverContent>
            <PopoverTitle>{field.label}</PopoverTitle>
            <Input
              aria-label="Search metrics"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search metrics…"
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
                  <legend className="mb-2 text-sm font-medium">{group}</legend>
                  <div className="space-y-2">
                    {items.map((option) => (
                      <div
                        key={option.value}
                        className="flex items-center gap-2"
                      >
                        <Checkbox
                          id={`${control.id}-${option.value}`}
                          checked={value.includes(option.value)}
                          disabled={disabled || option.disabled}
                          onCheckedChange={(checked) =>
                            onValueChange(
                              checked
                                ? [...new Set([...value, option.value])]
                                : value.filter((item) => item !== option.value),
                            )
                          }
                        />
                        <Label
                          htmlFor={`${control.id}-${option.value}`}
                          title={option.description}
                        >
                          {option.label}
                        </Label>
                      </div>
                    ))}
                  </div>
                </fieldset>
              ))}
            </div>
          </PopoverContent>
        </Popover>
      )}
    </FieldFrame>
  );
}
