import { configure } from "@testing-library/react";
import { createRequire } from "node:module";
import { budget } from "./budget";

// Default wait for findBy* queries and waitFor; explicit waits pass budget() themselves.
configure({ asyncUtilTimeout: budget(1000) });

// jsdom resolves `<label for>` by walking the whole tree, and `labels` (read by
// every accessible-name computation in `*ByRole` queries) resolves every label
// that way: quadratic in a large form. A connected label resolves through the
// document's id index instead, which also returns the first element in tree
// order with that id; other roots keep jsdom's walk.
if (typeof document !== "undefined") {
  const require = createRequire(import.meta.url);
  const {
    implementation: LabelImpl,
  } = require("jsdom/lib/jsdom/living/nodes/HTMLLabelElement-impl.js");
  const {
    isLabelable,
  } = require("jsdom/lib/jsdom/living/helpers/form-controls.js");
  const walk = Object.getOwnPropertyDescriptor(
    LabelImpl.prototype,
    "control",
  )!.get!;
  Object.defineProperty(LabelImpl.prototype, "control", {
    configurable: true,
    get() {
      const id = this.getAttributeNS(null, "for");
      const root = this.getRootNode({});
      if (!id || root.nodeType !== 9 /* DOCUMENT_NODE */)
        return walk.call(this);
      const target = root.getElementById(id);
      return target && isLabelable(target) ? target : null;
    },
  });
}
