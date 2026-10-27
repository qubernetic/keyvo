// What the Pad screen shows, folded from the daemon's `status` reply and the updates
// of a subscription, the way keyvo_client::Mirror folds them on the Rust
// side; and the one-line summary of a trace entry.
import type { Update } from "./daemon";
import { t } from "./i18n";
import { GLOBAL } from "./profiles";
import type {
  Action,
  Context,
  DeviceStatus,
  Geometry,
  Status,
  TraceRecord,
  View,
} from "./protocol";

/** The trace entries the Pad keeps and asks the daemon to replay. */
export const PAD_TRACE = 20;

/** The entries the collapsed trace strip shows on its one line. */
export const STRIP_LINE = 3;

/** The time of day of `ms` since the epoch, 24 h with seconds. */
export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  });
}

export interface PadState {
  view?: View;
  /** The profile whose page list the pad walks (spec §6.3): the view's
   * profile from index 1 on; at index 0 and below the view shows a global
   * page, and the owner stays the one seen before (the global profile when
   * none was). */
  owner?: string;
  device?: DeviceStatus;
  /** The last geometry a Keypad reported; it stays when the Keypad goes,
   * as the daemon keeps painting for it. */
  geometry?: Geometry;
  context?: Context;
  /** While `context` is held, the window reported before it, whose context
   * stays; none when this state saw no such report. */
  holding?: Context;
  /** The keys held down on the device, in key order. */
  held: number[];
  /** The newest trace entries, newest first. */
  trace: TraceRecord[];
  /** Why the last reload failed, until one succeeds. */
  configError?: string;
  /** Reloads seen, so the tiles are fetched again after each one. */
  reloads: number;
  /** Why the subscription ended; the rest is the last frame. */
  closed?: string;
  /** When the Keypad was last seen connected, in ms since the epoch; none
   * while it has not been seen since the app started. */
  seen?: number;
}

export function initial(): PadState {
  return { held: [], trace: [], reloads: 0 };
}

/** `state` showing `view`, with the list owner that follows from it. */
function showing(state: PadState, view: View): PadState {
  return { ...state, view, owner: view.index >= 1 ? view.profile : (state.owner ?? GLOBAL) };
}

/** The state of the `status` reply received at `now`. */
export function fromStatus(status: Status, now = Date.now()): PadState {
  return {
    ...showing(initial(), status.view),
    device: status.device,
    geometry: status.device.geometry,
    context: status.context,
    seen: status.device.connected ? now : undefined,
  };
}

/** The key numbers of `geometry` in rows, top to bottom, each row left to
 * right. A key starts a new row when its top edge is below the middle of
 * the row's first key, so a pixel of offset does not split a row. */
export function grid(geometry: Geometry): number[][] {
  const keys = geometry.rects
    .map((rect, index) => ({ rect, key: index + 1 }))
    .sort((a, b) => a.rect.y - b.rect.y);
  const rows: { middle: number; keys: typeof keys }[] = [];
  for (const entry of keys) {
    const row = rows.at(-1);
    if (row !== undefined && entry.rect.y < row.middle) {
      row.keys.push(entry);
    } else {
      rows.push({ middle: entry.rect.y + entry.rect.height / 2, keys: [entry] });
    }
  }
  return rows.map((row) => row.keys.sort((a, b) => a.rect.x - b.rect.x).map((entry) => entry.key));
}

/** The key the arrow key `arrow` moves to from `key` in `rows` (as `grid`
 * gives them), staying inside the grid. */
export function moved(rows: number[][], key: number, arrow: string): number {
  const row = rows.findIndex((keys) => keys.includes(key));
  if (row < 0) return key;
  const column = rows[row]?.indexOf(key) ?? 0;
  const [r, c] =
    arrow === "ArrowUp"
      ? [Math.max(row - 1, 0), column]
      : arrow === "ArrowDown"
        ? [Math.min(row + 1, rows.length - 1), column]
        : arrow === "ArrowLeft"
          ? [row, Math.max(column - 1, 0)]
          : [row, column + 1];
  const target = rows[r] ?? [];
  return target[Math.min(c, target.length - 1)] ?? key;
}

/** `state` after `update`, received at `now`; `state` itself stays as it
 * was. */
export function apply(state: PadState, update: Update, now = Date.now()): PadState {
  switch (update.kind) {
    case "subscribed":
      return { ...showing(state, update.view), closed: undefined };
    case "closed":
      return { ...state, closed: update.reason };
    case "event":
      break;
  }
  const event = update.event;
  switch (event.event) {
    case "key": {
      const held = state.held.filter((key) => key !== event.key);
      if (event.edge === "press") {
        held.push(event.key);
        held.sort((a, b) => a - b);
      }
      return { ...state, held };
    }
    case "profile":
    case "page":
    case "mode":
      return showing(state, {
        profile: event.profile,
        page: event.page,
        mode: event.mode,
        index: event.index,
      });
    case "device": {
      const { event: _, ...device } = event;
      return {
        ...state,
        device,
        geometry: device.geometry ?? state.geometry,
        seen: device.connected ? now : state.seen,
      };
    }
    case "context": {
      const { event: _, ...context } = event;
      const before = state.context;
      const holding =
        context.held !== true ? undefined : before !== undefined && before.held !== true ? before : state.holding;
      return { ...state, context, holding };
    }
    case "trace":
      return { ...state, trace: [event.entry, ...state.trace].slice(0, PAD_TRACE) };
    case "config":
      return {
        ...state,
        configError: event.reloaded ? undefined : event.error,
        reloads: state.reloads + 1,
      };
  }
}

/** While the focused window is held (`[focus].hold_classes`, the keyvo
 * app; #117), the line that says so and names the profile the pad keeps;
 * none otherwise. */
export function caption(state: PadState): string | undefined {
  const { context, view } = state;
  if (context?.held !== true || view === undefined) return undefined;
  return t("pad.held", { class: context.class, profile: view.profile });
}

/** Where a configuration file fails to load (docs/design/gui-ux.md §5.2). */
export interface SchemaError {
  /** Relative to the configuration directory. */
  file: string;
  line: number;
  column?: number;
  message: string;
}

/** The place in a reload error (`<file>:<line>[:<column>]: <message>`, as
 * the daemon writes a diagnostic); none when it names no place. */
export function schemaError(error: string): SchemaError | undefined {
  const match = /^([^:\n]+):(\d+)(?::(\d+))?: (.*)$/s.exec(error);
  if (match === null) return undefined;
  const [, file = "", line = "", column, message = ""] = match;
  return {
    file,
    line: Number(line),
    ...(column === undefined ? {} : { column: Number(column) }),
    message,
  };
}

/** What `action` does, in a few words; none for an unbound key. */
export function describeAction(action: Action | null | undefined): string | undefined {
  switch (action?.type) {
    case undefined:
      return undefined;
    case "chord":
      return t("action.chord", { keys: action.keys });
    case "exec":
      return t("action.exec", { cmd: action.cmd });
  }
}

function result(entry: TraceRecord): string {
  const result = entry.result;
  switch (result.status) {
    case "dry_run":
      return t("trace.dryRun");
    case "pending":
      return t("trace.running");
    case "exited":
      return t("trace.exited", { code: String(result.code), ms: String(result.duration_ms) });
    case "failed":
      return t("trace.failed", { error: String(result.error) });
    default:
      return result.status;
  }
}

/** Present parts joined into one comma-separated phrase. */
export function phrase(parts: (string | undefined)[]): string {
  return parts.filter((part) => part !== undefined).join(", ");
}

/** One line: the key, how it was pressed, the layer that supplied it, the
 * action and what came of it. */
export function summarize(entry: TraceRecord): string {
  const head = phrase([
    t(entry.trigger.press === "long" ? "trace.keyLong" : "trace.key", { key: entry.key }),
    entry.trigger.simulated ? t("trace.simulated") : undefined,
    entry.layer === undefined ? undefined : t("layer", { layer: entry.layer }),
  ]);
  return t("trace.line", { head, body: phrase([describeAction(entry.action), result(entry)]) });
}
