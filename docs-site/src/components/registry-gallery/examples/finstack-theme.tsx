"use client";
import type { ExampleProps } from "./props";

export function Example(_props: ExampleProps) {
  return (
    <div className="grid grid-cols-2 gap-4">
      <div className="border border-border bg-background p-4 text-foreground">
        Background and foreground
      </div>
      <div className="border border-border bg-card p-4 text-card-foreground">
        Card
      </div>
      <div className="bg-primary p-4 text-primary-foreground">Primary</div>
      <div className="bg-muted p-4 text-muted-foreground">Muted text</div>
      <p className="font-mono">0123456789 · −0.125</p>
      <p>IBM Plex Sans · Typography</p>
    </div>
  );
}
