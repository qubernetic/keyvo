// The pin (pad screen brief §5.2, decision Q2; #108 open point 7): which
// profile and page the inspector edits. Undefined follows the pad. The
// header's profile dropdown and the page tabs pin it, selecting a key pins
// it to what the pad shows, and from then on a view event never retargets
// it, so the inspector never changes under the user; Follow unpins.
import type { Target } from "./editor";
import type { View } from "./protocol";

export interface Pin {
  profile: string;
  /** Counted from 1. */
  page: number;
}

/** The pin, or undefined to follow the pad. */
export type Pinned = Pin | undefined;

/** The key the inspector edits: `key` of the pinned page, else of the
 * page the pad shows; none before the pad reports a view. */
export function target(pinned: Pinned, view: View | undefined, key: number): Target | undefined {
  const pin = pinned ?? (view === undefined ? undefined : { profile: view.profile, page: view.page });
  return pin === undefined ? undefined : { ...pin, key };
}

/** What the pad shows when it differs from the pin, for the "pad moved"
 * line; undefined while following or when they agree. */
export function difference(pinned: Pinned, view: View | undefined): Pin | undefined {
  if (pinned === undefined || view === undefined) return undefined;
  if (pinned.profile === view.profile && pinned.page === view.page) return undefined;
  return { profile: view.profile, page: view.page };
}

/** The pin once a key is selected: kept if set, else the pad's profile
 * and page. */
export function pinOnSelect(pinned: Pinned, view: View | undefined): Pinned {
  if (pinned !== undefined || view === undefined) return pinned;
  return { profile: view.profile, page: view.page };
}

/** Pins profile `id` on page 1, the context page. */
export function pinProfile(id: string): Pin {
  return { profile: id, page: 1 };
}

/** Moves the pin to page `page` of its profile. */
export function pinPage(pinned: Pin, page: number): Pin {
  return { ...pinned, page };
}

/** Unpins: the inspector follows the pad again. */
export function follow(): Pinned {
  return undefined;
}

/** The pin after the profile list was read again: unpinned when its
 * profile is no longer there. */
export function kept(pinned: Pinned, profiles: readonly { id: string }[]): Pinned {
  return pinned !== undefined && profiles.some((profile) => profile.id === pinned.profile)
    ? pinned
    : undefined;
}
