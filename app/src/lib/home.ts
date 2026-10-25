// What Home shows, folded from the daemon's `status` reply and the updates
// of a subscription, the way keyvo_client::Mirror folds them on the Rust
// side; and the one-line summary of a trace entry.
import type { Update } from "./daemon";
import type { Context, DeviceStatus, Status, TraceRecord, View } from "./protocol";

/** The trace entries Home keeps and asks the daemon to replay. */
export const HOME_TRACE = 20;

export interface HomeState {
  view?: View;
  device?: DeviceStatus;
  context?: Context;
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
}

export function initial(): HomeState {
  return { held: [], trace: [], reloads: 0 };
}

export function fromStatus(status: Status): HomeState {
  return {
    ...initial(),
    view: status.view,
    device: status.device,
    context: status.context,
  };
}

/** `state` after `update`; `state` itself stays as it was. */
export function apply(state: HomeState, update: Update): HomeState {
  switch (update.kind) {
    case "subscribed":
      return { ...state, view: update.view, closed: undefined };
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
      return { ...state, view: { profile: event.profile, page: event.page, mode: event.mode } };
    case "device": {
      const { event: _, ...device } = event;
      return { ...state, device };
    }
    case "context": {
      const { event: _, ...context } = event;
      return { ...state, context };
    }
    case "trace":
      return { ...state, trace: [event.entry, ...state.trace].slice(0, HOME_TRACE) };
    case "config":
      return {
        ...state,
        configError: event.reloaded ? undefined : event.error,
        reloads: state.reloads + 1,
      };
  }
}

function action(entry: TraceRecord): string {
  switch (entry.action?.type) {
    case undefined:
      return "";
    case "chord":
      return `chord ${entry.action.keys}, `;
    case "exec":
      return `exec ${entry.action.cmd}, `;
  }
}

function result(entry: TraceRecord): string {
  const result = entry.result;
  switch (result.status) {
    case "dry_run":
      return "dry run, nothing executed";
    case "pending":
      return "running";
    case "exited":
      return `exited ${String(result.code)} after ${String(result.duration_ms)} ms`;
    case "failed":
      return `failed: ${String(result.error)}`;
    default:
      return result.status;
  }
}

/** One line: the key, how it was pressed, the layer that supplied it, the
 * action and what came of it. */
export function summarize(entry: TraceRecord): string {
  const how = [
    entry.trigger.press === "long" ? " long" : "",
    entry.trigger.simulated ? ", simulated" : "",
    entry.layer === undefined ? "" : `, ${entry.layer} layer`,
  ].join("");
  return `Key ${entry.key}${how}: ${action(entry)}${result(entry)}`;
}
