// The daemon over the mock store, answering like the Tauri shell's status,
// tiles, outline, press and subscribe. The device is the fixture's
// connected Keypad; the focus moves on a timer through Konsole, VS Code and
// a window no profile matches, and the trace replays the fixture's entries.
import type { Daemon, Outline, Tile, Update } from "../daemon";
import type { KeyFields } from "../editor";
import type { Context, Event, Layer, TraceRecord, View } from "../protocol";
import { fixtureStatus, fixtureTrace, SNAPSHOTS } from "./data";
import { attempt } from "./profiles";
import type { MockProfile, Store } from "./store";

/** How often the subscription sends the next fixture trace entry. */
export const TRACE_MS = 2_000;
/** How often the mock's focus moves to the next window of FOCUS. */
export const FOCUS_MS = 10_000;
/** The trace ring buffer's size (the daemon's `[trace].ring` default). */
const RING = 200;

/** The windows the focus cycles through; the last matches no profile. */
const FOCUS: readonly Omit<Context, "source">[] = [
  { class: "org.kde.konsole", title: "~ : bash", pid: 4242 },
  { class: "code", title: "main.rs - keyvo - Visual Studio Code", pid: 4343 },
  { class: "org.kde.dolphin", title: "Home - Dolphin", pid: 4444 },
];

const GLOBAL = "global";

export interface MockDaemonOptions {
  /** Rejects `status` as if no daemon listened on the socket. */
  unreachable?: boolean;
}

/** The page named `context`, else page 1 (keyvo_core::state::context_page). */
function contextPage(profile: MockProfile | undefined): number {
  const index = profile?.pages.findIndex((page) => page.name === "context") ?? -1;
  return index < 0 ? 1 : index + 1;
}

/** The snapshot that stands in for a key showing `fields`. */
function snapshot(fields: KeyFields | undefined): string {
  if (fields?.icon !== undefined && fields.label !== undefined) return SNAPSHOTS.iconLabel;
  if (fields?.icon !== undefined) return SNAPSHOTS.icon;
  if (fields?.label !== undefined) return SNAPSHOTS.label;
  return SNAPSHOTS.background;
}

export function mockDaemon(store: Store, options: MockDaemonOptions = {}): Daemon {
  const status = fixtureStatus();
  const trace = fixtureTrace();
  const ring: TraceRecord[] = [];
  let focus = FOCUS.length - 1;
  let replayed = 0;

  function context(): Context {
    return { ...(FOCUS[focus] ?? { class: "", title: "" }), source: "watcher" };
  }

  /** The profile the focused window selects: the highest priority among
   * the matching ones, else the global profile (spec §6.2). */
  function view(): View {
    const { class: focused } = context();
    const matching = store
      .all()
      .filter((profile) => profile.id !== GLOBAL && profile.match.class.includes(focused))
      .sort((a, b) => b.priority - a.priority);
    const profile = matching[0] ?? store.find(GLOBAL);
    return { profile: profile?.id ?? GLOBAL, page: contextPage(profile), mode: "auto" };
  }

  /** Key `key` of page `page` of `id` with the layer that supplies it: the
   * page's own key, else the global profile's context page's. */
  function resolve(id: string, page: number, key: number): [KeyFields, Layer] | undefined {
    const own = store.page(id, page).keys.get(key);
    const layer: Layer = id === GLOBAL ? "global" : "app";
    if (own !== undefined) return [own, layer];
    if (layer === "global") return undefined;
    const global = store.find(GLOBAL);
    const under = global?.pages[contextPage(global) - 1]?.keys.get(key);
    return under === undefined ? undefined : [under, "global"];
  }

  /** The tiles' reason for an unknown profile, as keyvo_client::tiles says it. */
  function known(id: string) {
    if (store.find(id) === undefined) {
      throw `no profile \`${id}\` in the configuration directory`;
    }
  }

  function record(entry: TraceRecord) {
    ring.push(entry);
    if (ring.length > RING) ring.shift();
  }

  return {
    status: () =>
      options.unreachable === true
        ? Promise.reject(
            "cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock: Connection refused (os error 111)",
          )
        : attempt(() => ({
            ...structuredClone(status),
            view: view(),
            context: context(),
            trace: ring.length,
          })),
    outline: (id) =>
      attempt((): Outline => {
        known(id);
        const { fields, pages } = store.get(id);
        return {
          pages: pages.length,
          ...(fields.accent === undefined ? {} : { accent: fields.accent }),
        };
      }),
    tiles: (id, page, keys) =>
      attempt(() => {
        known(id);
        try {
          store.page(id, page);
        } catch (error) {
          throw (error as { message: string }).message;
        }
        return Array.from({ length: keys }, (_, index): Tile => {
          const found = resolve(id, page, index + 1);
          if (found === undefined) return { image: SNAPSHOTS.background };
          const [fields, layer] = found;
          return {
            image: snapshot(fields),
            ...(fields.label === undefined ? {} : { label: fields.label }),
            ...(fields.action === undefined ? {} : { action: structuredClone(fields.action) }),
            layer,
          };
        });
      }),
    press: (key, kind) =>
      attempt(() => {
        const { profile, page } = view();
        const found = resolve(profile, page, key);
        const action = found?.[0].action;
        const entry: TraceRecord = {
          timestamp_ms: Date.now(),
          trigger: { press: kind, simulated: true },
          profile,
          page,
          key,
          ...(found === undefined ? {} : { layer: found[1] }),
          action: action === undefined ? null : structuredClone(action),
          result: { status: action === undefined ? "unbound" : "dry_run" },
        };
        record(entry);
        return entry;
      }),
    subscribe(replay, listener) {
      const send = (event: Event) => listener({ kind: "event", event } satisfies Update);
      listener({ kind: "subscribed", view: view() });
      for (const entry of ring.slice(Math.max(0, ring.length - replay))) {
        send({ event: "trace", entry: structuredClone(entry) });
      }
      const traceTimer = setInterval(() => {
        const next = trace[replayed++ % trace.length];
        if (next === undefined) return;
        const entry = { ...structuredClone(next), timestamp_ms: Date.now() };
        record(entry);
        send({ event: "trace", entry });
      }, TRACE_MS);
      const focusTimer = setInterval(() => {
        focus = (focus + 1) % FOCUS.length;
        send({ event: "context", ...context() });
        send({ event: "profile", ...view() });
      }, FOCUS_MS);
      return Promise.resolve(() => {
        clearInterval(traceTimer);
        clearInterval(focusTimer);
      });
    },
  };
}
