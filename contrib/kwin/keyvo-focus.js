// SPDX-License-Identifier: Apache-2.0
//
// keyvo focus feed for KWin 6 (spec §6.2).
//
// keyvo-daemon loads this script itself through
// org.kde.kwin.Scripting.loadScript under the plugin name "keyvo-focus",
// and loads it again when KWin restarts; do not install it by hand. Before
// loading, the daemon fills in the @SERVICE@, @PATH@ and @INTERFACE@
// placeholders with its own D-Bus connection and private object.
//
// When a window is activated, and when the active window's caption changes,
// the script calls Activated(class, caption, pid) on that object: KWin's
// resourceClass, caption and pid, all three as strings. The pid travels as a
// string because callDBus would send a JavaScript number as a double; an
// unknown pid is the empty string. The script reports the active window once
// at load and then only on KWin's signals; it never polls.

const service = "@SERVICE@";
const path = "@PATH@";
const iface = "@INTERFACE@";

let watched = null;

function report(window) {
    if (!window) {
        return;
    }
    const pid = window.pid > 0 ? String(window.pid) : "";
    callDBus(service, path, iface, "Activated",
        String(window.resourceClass || ""), String(window.caption || ""), pid);
}

function captionChanged() {
    report(watched);
}

function activated(window) {
    if (watched) {
        watched.captionChanged.disconnect(captionChanged);
    }
    watched = window;
    if (watched) {
        watched.captionChanged.connect(captionChanged);
    }
    report(window);
}

workspace.windowActivated.connect(activated);
activated(workspace.activeWindow);
