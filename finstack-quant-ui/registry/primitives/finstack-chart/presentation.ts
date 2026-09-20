import type {
  ChartTextMeasurer,
  ChartTextMeasureOptions,
  ChartTheme,
  SceneLabel,
} from "@tanstack/charts";
export interface FigureText {
  title?: string;
  subtitle?: string;
  caption?: string;
  sources?: readonly { label: string; url?: string }[];
  /** Text at explicit figure fractions (0..1); offsets only affect presentation. */
  figureAnnotations?: readonly {
    text: string;
    x: number;
    y: number;
    dx?: number;
    dy?: number;
  }[];
}
export interface FigurePresentation {
  theme: ChartTheme;
  cellText: { light: string; dark: string; size: number };
  ramps: Record<"sequential" | "diverging", readonly string[]>;
  fontFamily: string;
  titleSize: number;
  bodySize: number;
  noteSize: number;
  spacing: number;
  measureText: ChartTextMeasurer;
}
/** Public ChartTextMeasurer adapter to browser glyph metrics; the package's DOM measurer is private. */
export function readPresentation(element: HTMLElement): FigurePresentation {
  const css = getComputedStyle(element);
  const token = (name: string) => css.getPropertyValue(`--${name}`).trim();
  const number = (name: string) => {
    const value = Number.parseFloat(token(name));
    if (!Number.isFinite(value) || value <= 0)
      throw new Error(`Missing figure token: ${name}`);
    return value;
  };
  const canvas = element.ownerDocument.createElement("canvas").getContext("2d");
  if (!canvas) throw new Error("Figure text measurement requires Canvas 2D");
  const measureText: ChartTextMeasurer = (text, options) => {
    const size = options.fontSize * options.fontScale;
    canvas.font = `${options.fontStyle} ${options.fontWeight ?? 400} ${size}px ${options.fontFamily}`;
    canvas.textAlign = options.anchor === "middle" ? "center" : options.anchor;
    canvas.textBaseline =
      options.baseline === "auto" ? "alphabetic" : options.baseline;
    canvas.direction = options.direction;
    canvas.letterSpacing = `${options.letterSpacing}px`;
    const metrics = canvas.measureText(text);
    return {
      x: -metrics.actualBoundingBoxLeft,
      y: -metrics.actualBoundingBoxAscent,
      width: metrics.actualBoundingBoxLeft + metrics.actualBoundingBoxRight,
      height:
        metrics.actualBoundingBoxAscent + metrics.actualBoundingBoxDescent,
    };
  };
  return {
    theme: {
      foreground: token("foreground"),
      muted: token("muted-foreground"),
      grid: token("border"),
      background: token("background"),
      palette: Array.from({ length: 6 }, (_, i) => token(`chart-${i + 1}`)),
    },
    cellText: {
      light: token("cell-light"),
      dark: token("cell-dark"),
      size: number("text-xs"),
    },
    ramps: {
      sequential: Array.from({ length: 9 }, (_, i) =>
        token(`sequential-${i + 1}`),
      ),
      diverging: ["negative", "neutral", "positive"].map((name) =>
        token(`diverging-${name}`),
      ),
    },
    fontFamily: css.fontFamily,
    titleSize: number("text-lg"),
    bodySize: number("text-base"),
    noteSize: number("text-sm"),
    spacing: number("spacing"),
    measureText,
  };
}
/** Only publication prose wraps here; native chart guides own axis measurement and tick policies. */
export function figureLayout(
  text: FigureText,
  width: number,
  height: number,
  presentation: FigurePresentation,
) {
  const { spacing, measureText, fontFamily } = presentation;
  const inset = spacing * 2;
  const available = width - inset * 2;
  if (!(available > 0) || !Number.isFinite(height) || height <= 0)
    throw new Error(
      "Figure dimensions must be positive and leave room for text",
    );
  const lines: SceneLabel[] = [];
  function block(
    value: string | undefined,
    y: number,
    size: number,
    weight = 400,
    role = "note",
  ) {
    if (!value) return y;
    const options: ChartTextMeasureOptions = {
      fontSize: size,
      fontWeight: weight,
      fontFamily,
      fontStyle: "normal",
      fontStretch: "normal",
      letterSpacing: 0,
      direction: "ltr",
      fontScale: 1,
      anchor: "start",
      baseline: "hanging",
    };
    const wrapped: string[] = [];
    // Preserve explicit paragraphs; break an overlong word only when it cannot fit alone.
    for (const paragraph of value.split("\n")) {
      let line = "";
      for (const word of paragraph.split(/\s+/)) {
        const candidate = line ? `${line} ${word}` : word;
        if (measureText(candidate, options).width <= available) {
          line = candidate;
          continue;
        }
        if (line) wrapped.push(line);
        line = "";
        for (const character of word) {
          if (
            line &&
            measureText(line + character, options).width > available
          ) {
            wrapped.push(line);
            line = "";
          }
          line += character;
        }
      }
      wrapped.push(line);
    }
    for (const line of wrapped) {
      lines.push({
        kind: "label",
        key: `figure-${role}-${lines.length}`,
        x: inset,
        y,
        text: line,
        anchor: "start",
        baseline: "hanging",
        fontSize: size,
        fontWeight: weight,
        style: {
          fill:
            role === "title"
              ? presentation.theme.foreground
              : presentation.theme.muted,
        },
      });
      y += size * 1.4;
    }
    return y + spacing;
  }
  let top = text.title || text.subtitle ? inset : 0;
  top = block(text.title, top, presentation.titleSize, 500, "title");
  top = block(text.subtitle, top, presentation.bodySize, 400, "subtitle");
  const start = lines.length;
  let bottom = block(
    text.caption,
    text.caption || text.sources?.length ? inset : 0,
    presentation.noteSize,
    400,
    "caption",
  );
  for (const source of text.sources ?? [])
    bottom = block(
      source.url ? `${source.label} — ${source.url}` : source.label,
      bottom,
      presentation.noteSize,
      400,
      "source",
    );
  if (bottom) bottom += inset;
  for (const line of lines.slice(start)) line.y += height - bottom;
  for (const annotation of text.figureAnnotations ?? []) {
    if (
      ![annotation.x, annotation.y].every(
        (value) => Number.isFinite(value) && value >= 0 && value <= 1,
      )
    )
      throw new Error("Figure annotation fractions must be in 0..1");
    lines.push({
      kind: "label",
      key: `figure-annotation-${lines.length}`,
      x: annotation.x * width + (annotation.dx ?? 0),
      y: annotation.y * height + (annotation.dy ?? 0),
      text: annotation.text,
      anchor: "start",
      baseline: "hanging",
      fontSize: presentation.noteSize,
      style: { fill: presentation.theme.foreground },
    });
  }
  if (height - top - bottom < 96)
    throw new Error(
      "Figure dimensions cannot fit the supplied headings and notes; increase the height",
    );
  return { top, bottom, lines };
}
