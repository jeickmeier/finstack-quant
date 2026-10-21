"""Single-public-path check over rustdoc JSON.

For every workspace library crate, walk the public module tree from the crate
root and record every public path at which each item is reachable. An item's
identity is its defining path from the rustdoc `paths` table, so re-exports of
the same item (within a crate or across crates) collapse onto one identity.

Reports:
  * intra-crate duplicates: an item reachable at >1 path inside its own crate
  * cross-crate re-exports: crate X publicly re-exports an item defined in
    another workspace crate (or a third-party crate)
  * renamed re-exports (`pub use a::B as C`) and glob re-exports
  * umbrella view: the `finstack_quant::...` namespace users import from, with
    items reachable at >1 umbrella path

Usage (rustdoc JSON needs nightly; hidden and private items must be included so
`pub use` chains through private or `#[doc(hidden)]` modules resolve):

    RUSTDOCFLAGS="-Z unstable-options --output-format json --document-private-items --document-hidden-items" \
      cargo +nightly-2026-08-14 doc --workspace --no-deps --lib \
        --exclude finstack-quant-py --exclude finstack-quant-wasm \
        --exclude finstack-quant-valuations-macros --exclude finstack-quant-test-utils \
        --exclude finstack-quant-valuations-test-support
    python3 scripts/check_single_path.py target/doc single_path_report.md

Path segments marked with a dagger are `#[doc(hidden)]` items.
"""

from collections import defaultdict
import json
from pathlib import Path
import sys

DOC = Path(sys.argv[1])
OUT = Path(sys.argv[2])
UMBRELLA = "finstack_quant"
WS_PREFIX = "finstack_quant"


def inner_kind(item):
    """Return the rustdoc item kind (`module`, `use`, `struct`, ...) of `item`."""
    inner = item.get("inner") or {}
    return next(iter(inner)) if isinstance(inner, dict) and inner else str(inner)


def mark(item):
    """Suffix for a path segment whose item is `#[doc(hidden)]`."""
    return "†" if "hidden" in json.dumps(item.get("attrs", [])) else ""


def is_public(item):
    """Return whether `item` has `pub` visibility."""
    return item.get("visibility") == "public"


class Crate:
    """Public namespace of one crate, built from its rustdoc JSON file."""

    def __init__(self, path):
        """Load the rustdoc JSON at `path` and walk its public module tree."""
        self.data = json.loads(path.read_text())
        self.index = self.data["index"]
        self.paths = self.data["paths"]
        root = self.index[str(self.data["root"])]
        self.name = root["name"]
        self.ext = {k: v["name"] for k, v in self.data["external_crates"].items()}
        # identity -> list of public paths in this crate
        self.reach = defaultdict(list)
        self.kind_of = {}
        self.renames = []  # (public path, identity)
        self.globs = []  # (module path, target identity)
        self.reexports = []  # (public path, identity, source crate)
        self.mounts = []  # umbrella only: (prefix, external crate name, glob)
        self.seen_modules = set()
        self.walk(str(self.data["root"]), [self.name])

    def identity(self, item_id):
        """Return `(defining path, defining crate)` for `item_id`, or `(None, None)`."""
        entry = self.paths.get(str(item_id))
        if entry is None:
            return None, None
        crate = self.name if entry["crate_id"] == 0 else self.ext.get(str(entry["crate_id"]), "?")
        return "::".join(entry["path"]), crate

    def record(self, item_id, path, kind):
        """Record that `item_id` is publicly reachable at the segment list `path`."""
        ident, _ = self.identity(item_id)
        if ident is None:
            ident = f"<{self.name}#{item_id}>"
        self.reach[ident].append("::".join(path))
        self.kind_of[ident] = kind

    def walk(self, mod_id, prefix):
        """Record every public item of module `mod_id` under `prefix`, recursing into submodules."""
        if mod_id in self.seen_modules:
            return
        self.seen_modules.add(mod_id)
        module = self.index[mod_id]["inner"]["module"]
        for child in module["items"]:
            item = self.index.get(str(child))
            if item is None or not is_public(item):
                continue
            kind = inner_kind(item)
            if kind == "use":
                self.walk_use(item["inner"]["use"], prefix, mark(item))
                continue
            if kind in ("impl",):
                continue
            name = item.get("name")
            if name is None:
                continue
            path = [*prefix, name + mark(item)]
            self.record(child, path, kind)
            if kind == "module":
                self.walk(str(child), path)

    def walk_use(self, use, prefix, hid=""):
        """Resolve a `pub use` under `prefix`; `hid` is the hidden-marker suffix for its name."""
        target = use.get("id")
        if target is None:
            return
        target = str(target)
        # follow `pub use` chains through private modules to the defining item
        hops = 0
        while target in self.index and inner_kind(self.index[target]) == "use" and hops < 32:
            nxt = self.index[target]["inner"]["use"].get("id")
            if nxt is None or self.index[target]["inner"]["use"].get("is_glob"):
                break
            target = str(nxt)
            hops += 1
        ident, src_crate = self.identity(target)
        local = target in self.index
        if use.get("is_glob"):
            self.globs.append(("::".join(prefix), ident or use["source"]))
            if local and inner_kind(self.index[target]) == "module":
                # glob of a local module: every public child gets this prefix too
                self.walk_glob_local(target, prefix)
            elif not local and ident is not None and "::" not in ident:
                self.mounts.append(("::".join(prefix), ident, True))
            return
        name = use["name"]
        path = [*prefix, name + hid]
        last = (ident or use["source"]).split("::")[-1]
        if name != last and not (not local and ident and "::" not in ident):
            self.renames.append(("::".join(path), ident or use["source"]))
        if not local:
            if ident is not None and "::" not in ident:
                # re-export of a whole external crate root (umbrella mount)
                self.mounts.append(("::".join(path), ident, False))
                return
            self.record(target, path, (self.paths.get(target) or {}).get("kind", "?"))
            self.reexports.append(("::".join(path), ident, src_crate))
            return
        item = self.index[target]
        kind = inner_kind(item)
        self.record(target, path, kind)
        if kind == "module":
            self.walk(target, path)

    def walk_glob_local(self, mod_id, prefix):
        """Record the public children of local module `mod_id` glob-imported at `prefix`."""
        module = self.index[mod_id]["inner"]["module"]
        for child in module["items"]:
            item = self.index.get(str(child))
            if item is None or not is_public(item):
                continue
            kind = inner_kind(item)
            if kind == "use":
                self.walk_use(item["inner"]["use"], prefix, mark(item))
                continue
            if kind == "impl" or item.get("name") is None:
                continue
            path = [*prefix, item["name"] + mark(item)]
            self.record(child, path, kind)
            if kind == "module":
                self.walk(str(child), path)


def main():
    """Load every workspace crate's rustdoc JSON and write the Markdown report."""
    crates = {}
    for f in sorted(DOC.glob("finstack_quant*.json")):
        c = Crate(f)
        crates[c.name] = c

    lines = []
    w = lines.append
    summary = []

    # global identity -> {crate: [paths]} over the per-crate namespaces
    for name, c in sorted(crates.items()):
        if name == UMBRELLA:
            continue
        dups = {k: v for k, v in c.reach.items() if len(set(v)) > 1}
        hidden_dups = sum(1 for v in dups.values() if any("†" in p for p in v))
        ws_re = [r for r in c.reexports if r[2] and r[2].startswith(WS_PREFIX)]
        tp_re = [r for r in c.reexports if not (r[2] and r[2].startswith(WS_PREFIX))]
        summary.append((
            name,
            len(c.reach),
            len(dups),
            hidden_dups,
            len(ws_re),
            len(tp_re),
            len(c.renames),
            len(c.globs),
        ))
        w(f"\n## {name}\n")
        w(
            f"public items: {len(c.reach)}  multi-path: {len(dups)}  "
            f"workspace re-exports: {len(ws_re)}  third-party re-exports: {len(tp_re)}  "
            f"renames: {len(c.renames)}  globs: {len(c.globs)}\n"
        )
        if dups:
            w("### multi-path items")
            for ident, ps in sorted(dups.items(), key=lambda kv: (-len(set(kv[1])), kv[0])):
                w(f"- [{c.kind_of[ident]}] {ident}")
                for p in sorted(set(ps)):
                    w(f"    - {p}")
        if ws_re:
            w("### re-exports of other workspace crates")
            for p, ident, _src in sorted(ws_re):
                w(f"- {p}  <-  {ident}")
        if tp_re:
            w("### re-exports of third-party crates")
            for p, ident, _src in sorted(tp_re):
                w(f"- {p}  <-  {ident}")
        if c.renames:
            w("### renamed re-exports (`as`)")
            for p, ident in sorted(c.renames):
                w(f"- {p}  <-  {ident}")
        if c.globs:
            w("### glob re-exports")
            for p, ident in sorted(c.globs):
                w(f"- {p}::*  <-  {ident}")

    # umbrella view: mount each crate's namespace at its umbrella prefix
    umb = crates.get(UMBRELLA)
    umbrella_dups = {}
    if umb:
        view = defaultdict(set)
        for ident, ps in umb.reach.items():
            for p in ps:
                view[ident].add(p)
        for prefix, crate_name, _glob in umb.mounts:
            c = crates.get(crate_name)
            if c is None:
                continue
            for ident, ps in c.reach.items():
                for p in ps:
                    rest = p.split("::", 1)[1] if "::" in p else ""
                    view[ident].add(f"{prefix}::{rest}" if rest else prefix)
        umbrella_dups = {k: v for k, v in view.items() if len(v) > 1}
        kinds = {}
        for c in crates.values():
            kinds.update(c.kind_of)
        w(f"\n## umbrella view (`{UMBRELLA}::...`)\n")
        w(f"items: {len(view)}  multi-path: {len(umbrella_dups)}\n")
        for ident, ps in sorted(umbrella_dups.items(), key=lambda kv: (-len(kv[1]), kv[0])):
            w(f"- [{kinds.get(ident, '?')}] {ident}")
            for p in sorted(ps):
                w(f"    - {p}")

    head = [
        "# Single-public-path check\n",
        "| crate | public items | multi-path | of which via doc(hidden) | ws re-exports | 3rd-party re-exports | renames | globs |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    tot = [0] * 7
    for row in summary:
        head.append("| " + " | ".join(str(x) for x in row) + " |")
        tot = [a + b for a, b in zip(tot, row[1:], strict=False)]
    head.append("| **total** | " + " | ".join(f"**{t}**" for t in tot) + " |")
    if umb:
        head.append(
            f"\nUmbrella `finstack_quant::` view: **{len(umbrella_dups)}** items reachable at more than one path."
        )
    OUT.write_text("\n".join(head + lines) + "\n")
    print("\n".join(head))


main()
