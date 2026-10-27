# Page navigation prior art: per-app profiles, pages, a global page, and the page buttons

Date: 2026-10-27. Scope: how the best-regarded control-surface apps combine
(a) per-application profiles that follow the focused window, (b) several pages
inside a profile, (c) a shared page or profile reachable from any app, and
(d) the physical page or back buttons; and which flow users call good. keyvo's
current model is spec §6.1 and §6.3. Reddit blocks the crawler used here, so
user voice comes from Automators Talk, MPU Talk, the Keyboard Maestro,
SoundFlow, Steinberg and BetterTouchTool forums, reviews, and the issue
trackers of the open-source projects; one Reddit thread is reached through an
Elgato issue that cites it [12].

## 1. Summary

1. Every product that follows the focused app has exactly one fallback for an
   app without a profile: Elgato's default profile [3][40], Logitech's General
   Profile [13], Loupedeck's Main Profile [20]. None of them has a "general
   page reachable from inside an app profile"; the shared layer is a profile.
2. Pages belong to a profile and the physical page controls walk that
   profile's pages, nothing else: Logitech Keypad (up to 15 pages, paging
   buttons or a "Navigate to page" key) [14][15], Stream Deck Neo (touch
   points only scroll pages) [9], Razer Stream Controller X (fixed corner
   keys) [26], Elgato pages 2 to 9 get Next and Previous keys [4].
3. Keys that must be on every page are solved by a layer, not by a page:
   Loupedeck's round buttons are workspace-independent [21][22],
   StreamController added sticky inputs after a request for a volume dial on
   all pages [29][30], BetterTouchTool users ask for always-active pages
   next to app pages [46].
4. "Back" is the recurring pain: users want to return to the previous profile
   or page, not to a fixed home [39][34], and a fixed back key in the best
   slot annoys them [43][11].
5. Users praise automatic switching itself [45][18] and physical page buttons
   that cost no keys [18][46]; they complain when the button that reached
   the second page is lost to something else [46] and when switching back is
   wrong or missing [40][42].
6. Recommendation (section 5): the page buttons page through the shown
   profile's own pages in both modes, with the global general page appended
   as the last page of every ring; manual mode becomes a pure focus lock.

## 2. How each product models it

### Elgato Stream Deck

- Smart Profiles: a profile linked to one application becomes active when
  that program has focus; linking is done in the Profiles tab through the
  Application drop-down; they only run while the software is minimized and
  do not work with UWP apps [1]. One profile per app and one app per profile;
  several layouts for the same app are meant to be pages in that profile [2].
- Default profile: the profile marked "Make this my default profile" loads
  when the software starts [3]; users report the deck falls back to it when
  focus leaves a smart-profile app [40] and when the linked app quits [41].
- Pages: up to 10 per profile; pages 2 to 9 receive Next Page and Previous
  Page actions that cannot be deleted but can be moved; Go To Page and Page
  Indicator actions exist [4]. Adding a page places a back arrow on the new
  page and a forward arrow on the first [5]. Folders do not count toward the
  page total [3].
- Folders: an action that opens another set of keys; the back arrow returns
  to the parent, folders nest, holding the Exit key returns to the main
  layout, an Auto Exit timer of up to 60 s returns on its own [6]. A folder
  has a single page [12]. The back key is fixed top-left; a third-party tool
  moves it, with a data-loss warning [11].
- Stream Deck +: the back key exits a folder [7]; a strip swipe pages in
  third-party software [8]. Neo: two touch points that only scroll pages and
  cannot be reassigned [9]; a review says they do not page inside folders
  [10]. Switch Profile: a key that jumps to a named profile, used as a home
  key [45][44].

### Logitech Options+ (MX Creative Console) and Loupedeck

- Three profile types: General Profile, "activated when there is no other
  active application profile"; Application Profile, activated when its app
  is open; imported profiles [13]. "Adapt to App" (the Focus to Application
  toggle) can be turned off to keep one set of actions fixed; then the
  profile is changed in the configuration UI [13].
- Keypad pages: added with the plus sign on the page selector below the
  device, reordered by drag, deleted by right-click; "in the absence of
  folders" the paging buttons or a "Navigate to page" action jump between
  pages [14]. Reviews count up to 15 pages per application [15][16].
- Actions Ring: an on-screen overlay installed from Options+ 1.83 on [17].
- Loupedeck (same lineage, now Logitech): Adapt to App, formerly Dynamic
  Mode, is on by default and follows the active app; off keeps the selected
  profile; a toggle action exists [19]. When the linked app closes the
  console returns to the Main Profile [20]. Workspaces are tool boxes inside
  a profile; exactly one is the home workspace, reached with the home button
  [21]. Round and square buttons are "General Elements" whose actions do not
  change with the workspace; home, enter, keyboard and Fn are fixed [22].
  The home button's first tap returns to the workspace's first page, a
  double tap lists the workspaces [23]. Pages are swiped or reached by page
  links [24]. A Switch Profile action exists [25].

### Razer Stream Controller X and Mountain DisplayPad

- Razer runs Loupedeck software; dynamic mode switches to app profiles. On
  the X the lower-left and lower-right keys are navigation, the right one
  only on the first page, the left one only on the last; they cannot be
  reprogrammed, only re-iconed, so 13 or 14 keys remain per page [26].
- Mountain Base Camp: profiles change manually or automatically when
  user-defined programs launch or close; a folder bound to a key adds another
  layer [27]; the profile can also be changed from the Display Dial [28].

### Linux open-source projects

- StreamController: automatic page switching by window title or class on
  GNOME, Hyprland, Sway, KDE (kdotool) and X11; pages, not profiles, are the
  unit; sticky actions put one key on all pages [29], added after a request
  for a volume dial on every page [30]. A user expected a fallback to a
  default page when no rule matches and found none [31].
- OpenDeck: switches a profile when a given app's window is active; profiles
  nest into folders by name prefix [32]; no pages inside a profile yet [33];
  "switch to the previous profile" or a stack of them is an open request
  [34].
- Boatswain: pages and profiles, Smart Profiles an open request [35][36].
  deckmaster: pages and navigation between decks, no automatic switching
  [37]. streamdeck-ui: button pages, in maintenance mode [38].

## 3. What users say

Returning to where you came from: "my problem is returning to the previous
profile" [39]; another wants pages that "go back up a page instead of always
to the home screen" [39]; OpenDeck's shared settings pages leave "no way to
get back to some profiles" [34]; pages inside folders are asked for
"consistent navigation and muscle memory" [12].

The fallback when an app has no profile: "the SD shows me the base one" [40];
"mine jumps back to the default profile when it comes off a smart profile",
so that user put a "back-to-default" key bottom-left on every profile [40];
with the default-profile option on, "once the app quits it will go back to
the default profile" [41]; a plugin's Main key sent a user to "a Windows
shortcuts profile" until he pointed it at the Cubase profile [42].

Global keys that must stay reachable: a volume dial that works "regardless of
which page I'm on" [30]; "always-active buttons and dials" on their own pages
next to app pages [46].

Physical page buttons versus keys: "I like how there are physical buttons for
page switching", on Stream Deck "I had to reserve two buttons" [18]; "losing
2 of the 8 buttons to page forward/back controls is a major loss" [46]; "I
don't know how many folks will step through 15 pages", three or four is the
reviewer's limit [15].

The fixed back key: it "is taking up the most valuable space" and dragging it
"snaps right back" [43]; one user places the parent folder top-left so the
"Back" lands "at the same location" [44].

Automatic switching itself: "I love the smart profiles" [45]; "Logitech's
implementation feels especially fluid" [18]; a Spotify profile that "doesn't
happen to me" [18]; a flight-sim user turned dynamic mode off and picked the
profile by hand [47].

## 4. Patterns that work and patterns that annoy

| Pattern | Seen in | Verdict | Evidence |
|---|---|---|---|
| App profile follows focus, one fixed fallback profile | Elgato, Logitech, Loupedeck | Works; users expect it | [1][13][20][40] |
| Dedicated page buttons that cost no keys | Logitech Keypad, Neo | Praised | [18][46] |
| Page buttons walk only the shown profile's pages | Logitech, Neo, Razer, Elgato | Works, predictable | [14][9][26][4] |
| Workspace-independent keys (a layer under every page) | Loupedeck round buttons, StreamController sticky | Works; requested where missing | [22][30][46] |
| A home or back key that returns to a known first page | Loupedeck home button | Works | [21][23] |
| Jump to a fixed home when the user wanted "previous" | Elgato Switch Profile, folders | Annoys | [39][34][42] |
| Fixed back key in the best slot | Elgato folders | Annoys | [43][11] |
| Pages that are not reachable the usual way (folders) | Elgato | Annoys | [12][10] |
| Silent return to default when focus leaves | Elgato | Mixed; needs a known rule | [40][41] |
| No fallback when nothing matches | StreamController | Annoys | [31] |
| Deep page rings (15 pages) | Logitech | Unused past 3 or 4 | [15] |

## 5. Options for keyvo

Today (spec §6.3): auto mode toggles context and the global general page on
either button; manual mode pages through the shown profile; a long press
toggles the mode; both buttons held escapes. The pain: VS Code's own second
page is reachable only in manual mode.

Option 1: the toggle prefers the shown profile's own "general" page when it
has one, else the global one.

- Borrows: Loupedeck's home workspace per profile [21].
- Costs: §6.1 "the general page is the global profile's page named general"
  becomes "the shown profile's, else the global's"; the state machine's page
  resolution changes in one place; `page_grace` and overlays are unchanged.
  Two new rules appear: what lies under an app's general page (today the
  global context page), and the global media keys vanish for every app that
  owns a general page, the exact complaint behind sticky inputs [30][46].
  The same button then means different things per app, and a third app page
  is still manual-only. Cheap, but it moves the pain rather than removing it.

Option 2: auto mode pages through every page of the shown profile; the
global general page is reached another way.

- Borrows: Logitech Keypad (page buttons cycle the app profile's pages, the
  General Profile is a fallback profile) [13][14], Neo and Razer (fixed page
  controls) [9][26].
- Costs: §6.1 `page : auto -> { context, general }` becomes a page index in
  both modes; §6.3 loses the auto-mode toggle rule; `page_grace` applies to
  any page other than context; the state table tests in
  `crates/keyvo-core/tests/state.rs` are rewritten. The open question is the
  "other way": a long press is taken by the mode toggle, both buttons held
  by the escape, and a key on every context page costs one of nine keys,
  which users reject [46]. Manual mode and auto mode then differ only in
  whether focus follows, so manual mode collapses into a focus lock,
  Loupedeck's "Adapt to App off" [19].

Option 3: a page ring with the global general page appended, both modes.

- The shown profile's ring is `[context, its other pages..., global general]`;
  the right button goes forward, the left back, both wrapping. From the
  context page one left press reaches the global general page, as today, and
  one right press reaches the app's second page. A profile without extra
  pages behaves exactly as now. The general page keeps its global stack
  (overlays never apply to it, §6.1). Manual mode is the focus lock only: the
  ring is the same, focus changes are recorded and not shown. An app switch
  returns to the context page unless a page button was used within
  `page_grace`, as today. Both buttons held stays the escape.
- Borrows: Logitech's per-profile page ring with a General fallback
  [13][14], Loupedeck's home that is always at the end of the back presses
  and its workspace-independent layer [21][22], StreamController's sticky
  layer [29], the Razer X's asymmetric corner keys [26].
- Costs: the same §6.1 and §6.3 rewrite as option 2, plus one rule, "the
  global general page is the last page of every ring"; no new chord, no key
  spent, no per-app meaning of a button. The state machine gains a page
  index in auto mode and loses the toggle. `general_hint` stays as the key on
  the general page that mirrors the top overlay. A profile may still name a
  page "general" of its own; it is then an ordinary page in the ring, which
  removes the special case option 1 would add.

Recommendation: option 3. It is option 2 with the "other way" answered by the
ring itself, so the pre-alpha habit survives (a left press from context is
the global general page) and the page buttons do what every shipping product
makes them do: walk the pages of what is shown. Option 1 is not recommended;
it hides the shared keys exactly where users ask for them to persist. The
GUI's page tabs already list a profile's pages, so the mirror needs no new
concept.

## 6. Sources

1. https://www.elgato.com/ww/en/explorer/products/stream-deck/how-to-set-up-smart-profiles-on-stream-deck/
2. https://arise-create.de/en/knowledge/stream-deck-smart-profiles
3. https://www.sweetwater.com/sweetcare/articles/elgato-stream-deck-quickstart-guide/
4. https://help.elgato.com/hc/en-us/articles/4410312027277-Elgato-Stream-Deck-Pages
5. https://www.sideshowfx.net/stream-deck-how-to-guide
6. https://www.elgato.com/us/en/explorer/products/stream-deck/how-to-use-folders-stream-deck/
7. https://www.elgato.com/us/en/explorer/products/stream-deck/elgato-stream-deck-plus-quick-start-guide/
8. https://companion.free/user-guide/v4.2/surfaces/elgato-streamdeck
9. https://www.tomshardware.com/peripherals/elgato-stream-deck-neo-review
10. https://www.wepc.com/reviews/elgato-stream-deck-neo
11. https://github.com/BarRaider/streamdeck-foldereditor
12. https://github.com/elgatosf/streamdeck/issues/119
13. https://support.logi.com/hc/en-ch/articles/25575505956119-Profiles-in-Logi-Options-MX-Creative-Console
14. https://support.logi.com/hc/en-us/articles/25407700552599-Settings-Configuration-MX-Creative-Console
15. https://www.provideocoalition.com/review-logitech-mx-creative-console/
16. https://www.notebookcheck.net/Logitech-MX-Creative-Console-hands-on-review-A-handy-content-creation-tool-that-takes-effort-to-configure.902574.0.html
17. https://support.logi.com/hc/ko/articles/17859293977111-Installing-Actions-Ring
18. https://futurework.blog/2025/06/17/logi-mx-creative-console/
19. https://support.loupedeck.com/dynamic-mode-on/off.html
20. https://support.loupedeck.com/what-is-an-application-profile.html
21. https://support.loupedeck.com/what-are-workspaces.html
22. https://support.loupedeck.com/round_and_square_buttons.html
23. https://support.loupedeck.com/ableton-live-workspaces.html
24. https://support.loupedeck.com/pages.html
25. https://support.loupedeck.com/switch-profile-action.html
26. https://www.tomshardware.com/reviews/razer-stream-controller-x
27. https://www.guru3d.com/articles-pages/mountain-display-and-macro-pad-for-keyboard-review,1.html
28. https://mountain.gg/news/mountain-releases-base-camp-v1.0.39/
29. https://github.com/StreamController/StreamController
30. https://github.com/StreamController/StreamController/issues/587
31. https://github.com/StreamController/StreamController/issues/215
32. https://github.com/nekename/OpenDeck
33. https://github.com/nekename/OpenDeck/issues/404
34. https://github.com/nekename/OpenDeck/issues/394
35. https://gitlab.gnome.org/World/boatswain
36. https://gitlab.gnome.org/World/boatswain/-/issues/64
37. https://github.com/muesli/deckmaster
38. https://github.com/streamdeck-linux-gui/streamdeck-linux-gui
39. https://talk.automators.fm/t/can-stream-deck-go-to-the-previously-used-profile/15246
40. https://talk.macpowerusers.com/t/yet-another-stream-deck-question/24471
41. https://forum.keyboardmaestro.com/t/change-stream-deck-profile-from-macro/15542?page=2
42. https://forums.steinberg.net/t/streamdeck-cubase-14-sideshow-fx-problems/963819
43. https://forum.soundflow.org/-5078/move-folder-back-button
44. https://talk.macpowerusers.com/t/stream-deck-xl-and-relay-fm/13338
45. https://switowski.com/blog/my-stream-deck-setup/
46. https://community.folivora.ai/t/please-add-stream-deck-paging-to-support-sd-and-neo/38123
47. https://forums.flightsimulator.com/t/loupedeck-live-ct-plugin/487200?page=13
