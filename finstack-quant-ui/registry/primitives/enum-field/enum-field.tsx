"use client";
import { Select } from "@base-ui/react/select";
import { Radio } from "@base-ui/react/radio";
import { RadioGroup } from "@base-ui/react/radio-group";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
export interface EnumOption {
  value: string;
  label: string;
  description?: string;
  group?: string;
  disabled?: boolean;
}
export interface EnumFieldProps extends FieldInfo {
  value: string;
  onValueChange: (value: string) => void;
  options: readonly EnumOption[];
  disabled?: boolean;
  layout?: "select" | "radio";
}
/** Controlled enum choices supplied by the canonical caller, including variant descriptions. */
export function EnumField({
  value,
  onValueChange,
  options,
  disabled,
  layout = options.length <= 4 ? "radio" : "select",
  ...field
}: EnumFieldProps) {
  const groups = Map.groupBy(options, (option) => option.group ?? "");
  return (
    <FieldFrame {...field}>
      {(control) =>
        layout === "radio" ? (
          <RadioGroup
            {...control}
            value={value}
            onValueChange={(next) => onValueChange(next)}
            disabled={disabled}
            className="flex flex-wrap gap-3"
          >
            {options.map((option) => (
              <label
                key={option.value}
                title={option.description}
                className="flex items-center gap-1 text-sm"
              >
                <Radio.Root
                  value={option.value}
                  disabled={option.disabled}
                  className="size-4 rounded-full border border-border data-checked:border-primary focus-visible:outline-2 focus-visible:outline-ring"
                >
                  <Radio.Indicator className="m-auto block size-2 rounded-full bg-primary" />
                </Radio.Root>
                {option.label}
              </label>
            ))}
          </RadioGroup>
        ) : (
          <Select.Root
            value={value || null}
            onValueChange={(next) => onValueChange(next ?? "")}
            disabled={disabled}
            items={options}
          >
            <Select.Trigger
              {...control}
              className="finstack-field flex w-full items-center justify-between rounded-sm border border-border bg-background px-2 text-left text-sm focus-visible:outline-2 focus-visible:outline-ring"
            >
              <Select.Value placeholder="Select…" />
              <Select.Icon>⌄</Select.Icon>
            </Select.Trigger>
            <Select.Portal>
              <Select.Positioner sideOffset={4}>
                <Select.Popup className="max-h-72 min-w-[var(--anchor-width)] overflow-auto rounded-md border border-border bg-card p-1 text-card-foreground shadow-[var(--elevation)]">
                  <Select.List>
                    {[...groups].map(([group, items]) => (
                      <Select.Group key={group}>
                        {group && (
                          <Select.GroupLabel className="p-2 text-xs text-muted-foreground">
                            {group}
                          </Select.GroupLabel>
                        )}
                        {items.map((option) => (
                          <Select.Item
                            key={option.value}
                            value={option.value}
                            disabled={option.disabled}
                            className="rounded-sm p-2 text-sm data-highlighted:bg-accent data-highlighted:text-accent-foreground"
                          >
                            <Select.ItemText>{option.label}</Select.ItemText>
                            {option.description && (
                              <span className="block max-w-sm line-clamp-2 whitespace-pre-wrap text-xs text-muted-foreground">
                                {option.description}
                              </span>
                            )}
                          </Select.Item>
                        ))}
                      </Select.Group>
                    ))}
                  </Select.List>
                </Select.Popup>
              </Select.Positioner>
            </Select.Portal>
          </Select.Root>
        )
      }
    </FieldFrame>
  );
}
