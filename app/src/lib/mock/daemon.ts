// The daemon over the mock store, answering like the Tauri shell's status,
// tiles, outline, recipes, press and subscribe. The recipe packs are the
// fixture's answer, the packs of contrib/recipes/. The device is the fixture's
// connected Keypad; the focus moves on a timer through Konsole, the keyvo
// app (held, as `[focus].hold_classes` holds it), VS Code and a window no
// profile matches, and the trace replays the fixture's entries. The Dialpad
// is the fixture's connected one, its dial and roller turning on a timer.
// The modifier keys of the contrib profiles (#189) take the dial on a timer
// too: global key 7 held, then key 8 toggled, each for one step; a press,
// a dry run, changes no modifier, as the daemon's does not.
import type { Daemon, Outline, Tile, Update } from "../daemon";
import { modifierOf, type KeyFields } from "../editor";
import type { Context, DialpadStatus, Event, Layer, Modifier, TraceRecord, View } from "../protocol";
import { fixtureDialpad, fixtureRecipes, fixtureStatus, fixtureTrace, SNAPSHOTS } from "./data";
import { attempt } from "./profiles";
import { contextPage, type Store } from "./store";

/** How often the subscription sends the next fixture trace entry. */
export const TRACE_MS = 2_000;
/** How often the mock's focus moves to the next window of FOCUS. */
export const FOCUS_MS = 10_000;
/** How often the mock's connected Dialpad turns a rotary, the dial and
 * the roller in turn, so the panel's pulse shows. */
export const DIAL_MS = 3_000;
/** How often the mock's modifier moves to the next step of MODIFIERS. */
export const MODIFIER_MS = 5_000;
/** The trace ring buffer's size (the daemon's `[trace].ring` default). */
const RING = 200;

/** The modifier steps in turn: key 7 held, none, key 8 toggled, none. A
 * step whose key is no modifier key of that mode on the shown page is
 * none. */
const MODIFIERS: readonly (Modifier | null)[] = [
  { key: 7, mode: "held" },
  null,
  { key: 8, mode: "toggle" },
  null,
];

/** The turns the Dialpad makes in turn: two detents of the dial
 * clockwise, one of the roller back. */
const TURNS: readonly Omit<Event & { event: "dial" }, "event">[] = [
  { roller: "dial", delta: 2, actions: 0 },
  { roller: "roller", delta: -1, actions: 0 },
];

/** The windows the focus cycles through; the keyvo app is held, so the
 * view stays the one of the window before it; the last matches no
 * profile. */
const FOCUS: readonly Omit<Context, "source">[] = [
  { class: "org.kde.konsole", title: "~ : bash", pid: 4242 },
  { class: "keyvo-app", title: "keyvo", pid: 4545, held: true },
  { class: "code", title: "main.rs - keyvo - Visual Studio Code", pid: 4343 },
  { class: "org.kde.dolphin", title: "Home - Dolphin", pid: 4444 },
];

const GLOBAL = "global";

export interface MockDaemonOptions {
  /** Rejects `status` as if no daemon listened on the socket. */
  unreachable?: boolean;
  /** The session is locked: the view says so, presses are blocked. */
  locked?: boolean;
  /** A Dialpad is connected (the default); false reports none
   * (`?mock-dialpad=none`). */
  dialpad?: boolean;
}

/** The key the lock page shows the lock on: the middle one. */
const LOCK_KEY = 5;

/** The snapshot that stands in for a key showing `fields`. */
function snapshot(fields: KeyFields | undefined): string {
  if (fields?.icon !== undefined && fields.label !== undefined) return SNAPSHOTS.iconLabel;
  if (fields?.icon !== undefined) return SNAPSHOTS.icon;
  if (fields?.label !== undefined) return SNAPSHOTS.label;
  return SNAPSHOTS.background;
}

export function mockDaemon(store: Store, options: MockDaemonOptions = {}): Daemon {
  const status = fixtureStatus();
  const dialpad: DialpadStatus =
    options.dialpad === false ? { connected: false } : fixtureDialpad();
  const trace = fixtureTrace();
  let turned = 0;
  const recipes = fixtureRecipes();
  const ring: TraceRecord[] = [];
  let focus = FOCUS.length - 1;
  /** The last focused window that is not held: it selects the view. */
  let selecting = focus;
  let replayed = 0;
  /** The time of the last replayed press, which its on_fail run keeps. */
  let pressedAt = 0;
  /** The modifier key that has the dial; none until the timer gives one. */
  let modifier: Modifier | null = null;
  let modified = 0;

  function context(): Context {
    return { ...(FOCUS[focus] ?? { class: "", title: "" }), source: "watcher" };
  }

  /** The profile the focused window selects: the highest priority among
   * the matching ones, else the global profile (spec §6.2). */
  function view(): View {
    const focused = FOCUS[selecting]?.class ?? "";
    const matching = store
      .all()
      .filter((profile) => profile.id !== GLOBAL && profile.match.class.includes(focused))
      .sort((a, b) => b.priority - a.priority);
    const profile = matching[0] ?? store.find(GLOBAL);
    return {
      profile: profile?.id ?? GLOBAL,
      page: contextPage(profile),
      mode: "auto",
      index: 1,
      locked: options.locked === true,
      // Absent while no key has the dial, as the daemon writes it.
      ...(modifier === null ? {} : { modifier: { ...modifier } }),
    };
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
            dialpad: structuredClone(dialpad),
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
    tiles: (id, page, keys, _tile, locked) =>
      attempt(() => {
        known(id);
        try {
          store.page(id, page);
        } catch (error) {
          throw (error as { message: string }).message;
        }
        if (locked === true) {
          return Array.from({ length: keys }, (_, index): Tile => ({
            image: index + 1 === LOCK_KEY ? SNAPSHOTS.lock : SNAPSHOTS.bare,
          }));
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
    recipes: () => attempt(() => structuredClone(recipes)),
    press: (key, kind) =>
      attempt(() => {
        const { profile, page } = view();
        if (options.locked === true) {
          const blocked: TraceRecord = {
            timestamp_ms: Date.now(),
            trigger: { press: kind, simulated: true },
            profile,
            page,
            key,
            action: null,
            result: { status: "blocked", reason: "session locked" },
          };
          record(blocked);
          return blocked;
        }
        const found = resolve(profile, page, key);
        // A short press of a toggle key arms or ends it and runs nothing;
        // a dry run only says so.
        const toggle = kind === "short" && found !== undefined && modifierOf(found[0]) === "toggle";
        const action = toggle ? undefined : found?.[0].action;
        const entry: TraceRecord = {
          timestamp_ms: Date.now(),
          trigger: { press: kind, simulated: true },
          profile,
          page,
          key,
          ...(found === undefined ? {} : { layer: found[1] }),
          ...(toggle ? { modifier: "toggle" as const } : {}),
          action: action === undefined ? null : structuredClone(action),
          result: { status: action === undefined && !toggle ? "unbound" : "dry_run" },
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
        // An on_fail run is a second entry of the press before it.
        if (next.branch !== "on_fail") pressedAt = Date.now();
        const entry = { ...structuredClone(next), timestamp_ms: pressedAt };
        record(entry);
        send({ event: "trace", entry });
      }, TRACE_MS);
      const focusTimer = setInterval(() => {
        focus = (focus + 1) % FOCUS.length;
        send({ event: "context", ...context() });
        // A held window leaves the view as it is: no profile event.
        if (FOCUS[focus]?.held === true) return;
        selecting = focus;
        // Another profile ends the modifier.
        modifier = null;
        send({ event: "profile", ...view() });
      }, FOCUS_MS);
      const modifierTimer = setInterval(() => {
        const step = MODIFIERS[modified++ % MODIFIERS.length] ?? null;
        const { profile, page } = view();
        const found = step === null ? undefined : resolve(profile, page, step.key);
        // Under the lock no key takes the dial.
        const next =
          options.locked !== true && found !== undefined && modifierOf(found[0]) === step?.mode
            ? step
            : null;
        if (next === null && modifier === null) return;
        modifier = next;
        send({ event: "modifier", modifier: next === null ? null : { ...next } });
      }, MODIFIER_MS);
      const dialTimer = dialpad.connected
        ? setInterval(() => {
            const turn = TURNS[turned++ % TURNS.length];
            if (turn !== undefined) send({ event: "dial", ...turn });
          }, DIAL_MS)
        : undefined;
      return Promise.resolve(() => {
        clearInterval(traceTimer);
        clearInterval(focusTimer);
        clearInterval(modifierTimer);
        clearInterval(dialTimer);
      });
    },
  };
}
