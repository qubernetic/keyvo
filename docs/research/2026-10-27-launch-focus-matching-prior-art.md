# Prior art: matching a launched application to its window

Date: 2026-10-27. Scope: how launchers, shells, compositors and control
surfaces decide that a newly focused window belongs to the application they
started a moment ago, and what a daemon without a window of its own can use
across Linux (KDE Wayland, X11), Windows and macOS. Input to the design of
the `apps` page behaviour of the `launch` action (spec §6.3, §8; the pending
launch list with `[timing].launch_focus`).

The question in two sentences: when a `launch` key on the `apps` page opens
an application, the app switch that this launch causes must keep the page
list index on `apps` (profile switches, index stays, the page return clock
restarts), while an app switch caused by the mouse must show the context
page as today. Has anyone solved "which window is the one I launched"
better than a class match with a timeout, and is there one way that works
on every platform keyvo targets?

Method: primary sources only where they exist (protocol XML, compositor and
shell source on invent.kde.org and gitlab.gnome.org, GLib and Flatpak
source, vendor documentation), read on 2026-10-27 from the `master` or
`main` branches. The owner's machine runs KWin 6.7.4 with the default focus
stealing prevention level (1, "Low").

## 1. Freedesktop startup notification (X11)

What it is. The launcher mints an id (`<launcher>-<pid>-<host>_TIME<ts>`),
puts it into the child's environment as `DESKTOP_STARTUP_ID`, and broadcasts
a `new:` message on the X root window with `ID`, `NAME`, `SCREEN`, optionally
`WMCLASS` (from the `StartupWMClass` desktop key). The toolkit in the child
reads the variable, unsets it, and sets the `_NET_STARTUP_ID` property on the
first window it maps; the window manager ends the sequence when a window
with that property, or with the announced WM class, appears, or after a
timeout (the spec asks for at least a minute; mutter uses 15 s). The
`StartupNotify` key in the desktop entry says whether the application does
any of this; `StartupWMClass` is the escape hatch for applications that do
not, and is defined as "it is known that the application will map at least
one window with the given string as its WM class".

Platforms. X11 only; XWayland windows still carry `_NET_STARTUP_ID`, so a
Wayland compositor can read it for X11 clients.

Launcher side. Mint the id, set the environment, send `new:`; KIO's
`KProcessRunner` does exactly this on X11 (`KStartupInfo::sendStartup`, the
pid added with `sendChange` once known, `sendFinish` when the process
ends), and checks `StartupNotify` and `StartupWMClass` first. GLib sets
both `DESKTOP_STARTUP_ID` and `XDG_ACTIVATION_TOKEN` to the same value in
the child's environment when the desktop entry has `StartupNotify=true` and
the launch context can mint an id.

Compositor side. mutter keeps `MetaStartupSequence` objects with id, app
id, WM class and timestamp, polls a 1 s timer, and completes sequences
whose elapsed time passes `STARTUP_TIMEOUT_MS` (15000). KWin on X11 uses
`KStartupInfo` from KWindowSystem.

Which window used it. This is the one mechanism where the window itself
carries the launch id: an X11 client can read `_NET_STARTUP_ID` on the newly
active window and compare it with the id it minted. GNOME Shell's window
tracker does this as its fifth rule (after WM class, sandbox id,
GApplication id and pid), walking the live sequences for an id equal to the
window's `_NET_STARTUP_ID`.

Flatpak. `flatpak run` builds the sandbox environment from the full host
environment (`flatpak_run_app` calls `flatpak_bwrap_new(NULL)`, which copies
`g_get_environ()`), then applies its own exports and the application's
`[Environment]` overrides; no Flatpak source names `DESKTOP_STARTUP_ID` or
`XDG_ACTIVATION_TOKEN`, so both reach the sandboxed process unless the user
unsets them with `--unset-env`.

Single instance. The id lands in the environment of the process the
launcher started; if that process hands over to a primary instance and
exits, the primary's window carries the id only when the toolkit forwards
it (GApplication passes `desktop-startup-id` and `activation-token` in the
D-Bus `platform_data`, GLib's `gdesktopappinfo.c` fills both keys). Apps
with their own single-instance logic do not.

Sources:
https://specifications.freedesktop.org/startup-notification-spec/latest/ ,
https://gitlab.gnome.org/GNOME/mutter/-/raw/main/src/core/startup-notification.c ,
https://invent.kde.org/frameworks/kio/-/raw/master/src/gui/kprocessrunner.cpp ,
https://gitlab.gnome.org/GNOME/glib/-/raw/main/gio/gdesktopappinfo.c ,
https://github.com/flatpak/flatpak/raw/main/common/flatpak-run.c ,
https://github.com/flatpak/flatpak/raw/main/common/flatpak-bwrap.c .

## 2. xdg-activation-v1 (Wayland)

What it is. The Wayland successor of startup notification, written by KDE
and GNOME developers (staging, interface version 1). A client asks
`xdg_activation_v1.get_activation_token`, optionally sets `app_id`,
`surface` and an input `serial` on the token object, commits it, and gets
a token string in the `done` event. It hands the string to the launched
application, by convention in `XDG_ACTIVATION_TOKEN` (the child must unset
it after reading) or as `activation-token` in the `platform_data` of an
`org.freedesktop.Application` D-Bus call. The launched application calls
`activate(token, surface)`. The compositor alone decides whether the token
is good; the protocol says a token can be ineffective from the start, for
example when the requester was not focused, and the requester cannot tell.
A token is single use. The protocol has no event that tells the requester
which surface used its token.

KWin (owner's compositor). `XdgActivationV1Integration::requestToken`
grants a token to a non-privileged client only if there is no active
window, or the requesting surface is the active window's surface, or the
serial is at least the last interaction serial and not in the future;
otherwise it returns the literal string `not-granted-666` and creates no
feedback. "Privileged" is one thing: the client's executable is
`plasmashell`. A granted token with `StartupNotify` creates an
`org_kde_plasma_activation` object (app id, then `finished`), the private
Plasma startup feedback that the task manager consumes. `activateSurface`
activates the window when `Workspace::mayActivate(window, token)` agrees,
else the window only demands attention; a surface that is not yet ready
stores the token and is judged when it maps. `mayActivate` returns true
when there is no active window, the level is None, the window is a
transient of the active one, the token equals the last granted token and
no input arrived since it was minted, or (below Extreme) the window's
`desktopFileName` equals the app id of the last granted token. At Extreme
only a proper token activates. Independently of all this, `addWindow`
activates every new window when the focus stealing prevention level is at
most Low, which is the default (`kwin.kcfg`:
`FocusStealingPreventionLevel` default 1). That is why keyvo's `sh -c`
launches take focus today without any token.

mutter (GNOME). `token_commit` turns the token into a
`MetaStartupSequence` with the token's app id and a timestamp and adds it
to the same 15 s startup notification machinery as on X11. On `activate`,
`maybe_activate` activates the window when the token "can activate" (its
seat, surface and serial describe a valid grab, so only a client with a
focused surface) or, for a token without such a surface, when the
sequence's timestamp is at or after the last user input; otherwise the
window demands attention. mutter does not write the token onto the
Wayland window; GNOME Shell's tracker matches windows by WM class,
sandbox id, GApplication id and pid before it looks at startup ids.

Which window used it. Neither compositor tells a third party. KWin's
scripting API exposes no activation signal; the only observable is the
Plasma activation feedback object (app id and `finished`), which exists
only for granted tokens with `StartupNotify`. For a daemon with no surface
and no input serial, KWin never grants a token, so on KDE Wayland the
token neither helps focus nor identifies the window.

Toolkits and tools. KWindowSystem reads `XDG_ACTIVATION_TOKEN` from the
environment and unsets it; Qt 6 and GTK 4 use it for `activate`. Firefox
added support in 2022 (bug 1767546). `gtk-launch` mints a token through
the GDK launch context; `gio launch` uses a plain `GAppLaunchContext`,
whose default `get_startup_notify_id` returns `NULL`, so it mints nothing;
`gapplication launch` goes through D-Bus activation with `platform_data`.
KIO's `ApplicationLauncherJob` asks `KWaylandExtras::xdgActivationToken`
with the launcher's window, writes the token into the child's environment,
and uses the terminal emulator's app id when launching in a terminal,
"otherwise the appId of the activation token doesn't match the actual
window" (`kprocessrunner.cpp`, line 450). KDE bug 446272 records that
`waitForStarted()` returned before the token arrived, so the pid was not
known yet.

Flatpak and single instance. As in section 1: the environment passes
through `flatpak run`; D-Bus activation carries the token in
`platform_data`; applications with their own single-instance handoff drop
it.

Sources:
https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/staging/xdg-activation/xdg-activation-v1.xml ,
https://invent.kde.org/plasma/kwin/-/raw/master/src/xdgactivationv1.cpp ,
https://invent.kde.org/plasma/kwin/-/raw/master/src/activation.cpp ,
https://invent.kde.org/plasma/kwin/-/raw/master/src/workspace.cpp ,
https://invent.kde.org/plasma/kwin/-/raw/master/src/kwin.kcfg ,
https://gitlab.gnome.org/GNOME/mutter/-/raw/main/src/wayland/meta-wayland-activation.c ,
https://invent.kde.org/frameworks/kwindowsystem/-/raw/master/src/platforms/wayland/windowsystem.cpp ,
https://gitlab.gnome.org/GNOME/glib/-/raw/main/gio/gappinfo.c ,
https://gitlab.gnome.org/GNOME/glib/-/raw/main/gio/gio-tool-launch.c ,
https://gitlab.gnome.org/GNOME/gtk/-/raw/main/tools/gtk-launch.c ,
https://bugzilla.mozilla.org/show_bug.cgi?id=1767546 ,
https://bugs.kde.org/show_bug.cgi?id=446272 ,
https://invent.kde.org/libraries/plasma-wayland-protocols/-/raw/master/src/protocols/plasma-window-management.xml .

## 3. Compositor-side application identity (app id, desktop file, pid)

What it is. Every compositor derives an application identity for each
window and matches launches to windows through it, not through tokens.

KWin. For an `xdg_toplevel`, `handleAppIdChanged` takes the client's
`app_id`; if the client connected through a security context
(`wp_security_context_v1`, which Flatpak sets with engine `org.flatpak`,
the app id and the instance id) and the `app_id` does not start with that
sandbox id, the sandbox id wins. The result is written to both
`resourceClass` and `desktopFileName`; window rules can override the
latter. `Window::findDesktopFile` resolves it to a `.desktop` path. The
window's `pid` is the Wayland client's socket credential
(`surface->client()->processId()`), which is the host pid even for a
process inside Flatpak's pid namespace, because the kernel translates
`SO_PEERCRED` into the receiver's namespace. A window without an app id
gets the executable's file name as class. The scripting API exposes
`desktopFileName`, `resourceClass`, `resourceName` and `pid` on `Window`,
with `desktopFileNameChanged` and `workspace.windowActivated`; keyvo's
`contrib/kwin/keyvo-focus.js` already forwards `resourceClass`, `caption`
and `pid`.

Plasma task manager (libtaskmanager). Window to launcher:
`serviceFromMetadata(appId, pid, wmClass)` tries `StartupWMClass` against
the app id and the X11 class, then the app id as a path, as
`DesktopEntryName`, as `Name`, then `/proc/<pid>/environ` (desktop file
hints left by launchers), then the process command line; a commit from
2019 prefers the pid match even when the desktop entry disagrees because
flatpak-builder can rename app ids. Startup to window on Wayland:
`WaylandStartupTasksModel` binds the Plasma activation feedback, maps each
activation's app id to `KService::serviceByDesktopName`, drops it on
`finished` or after the taskbar's `Timeout` (default 5 s), and
`TasksModel::filterAcceptsRow` hides a startup as soon as a window task
with the same `AppId` or `AppName` exists. Plasma's own answer to "which
window belongs to the launch" is therefore an app id match with a 5 s
timeout.

GNOME Shell. `get_app_for_window` walks: transient parent, WM_CLASS
instance then class against `StartupWMClass` and desktop ids, the
sandboxed app id (from the client's `.flatpak-info`), `_GTK_APPLICATION_ID`,
the window's pid against running apps, the startup sequence id, the X11
window group, and finally a fake app for the window. An app enters the
`STARTING` state when a sequence with its id exists and `RUNNING` when a
window maps.

Pitfalls. `_NET_WM_PID` is set by the application and is wrong inside a
pid namespace; KWindowSystem switched its X11 pid lookup to the XRES
extension for that reason, and KDE bug 473780 shows the symptom. Snap does
not use a pid namespace (processes stay in the global pid namespace), so
its pids match. Flatpak's `--unshare-pid` makes bwrap pid 1 of the sandbox;
the host can still open `/proc/<hostpid>/root/.flatpak-info` to read the
app id and instance id (the Flatpak source keeps that file unfakable for
exactly this use). Flatpak also moves each instance into a transient
systemd scope named `app-flatpak-<appid>-<instanceid>.scope`, so
`/proc/<pid>/cgroup` names the app as well. KDE bug 459986 shows the
other failure: a terminal application launched with its own desktop entry
maps a window whose app id is the terminal's, so an app id match never
completes.

Sources:
https://invent.kde.org/plasma/kwin/-/raw/master/src/xdgshellwindow.cpp ,
https://invent.kde.org/plasma/kwin/-/raw/master/src/waylandwindow.cpp ,
https://invent.kde.org/plasma/kwin/-/raw/master/src/window.cpp ,
https://develop.kde.org/docs/plasma/kwin/api/ ,
https://github.com/flatpak/flatpak/raw/main/common/flatpak-run-wayland.c ,
https://invent.kde.org/plasma/plasma-workspace/-/raw/master/libtaskmanager/tasktools.cpp ,
https://invent.kde.org/plasma/plasma-workspace/-/raw/master/libtaskmanager/waylandstartuptasksmodel.cpp ,
https://invent.kde.org/plasma/plasma-workspace/-/raw/master/libtaskmanager/tasksmodel.cpp ,
https://phabricator.kde.org/R119:4a6a518a8b4c8aec7968f3698191cc2daafcdec4 ,
https://gitlab.gnome.org/GNOME/gnome-shell/-/raw/main/src/shell-window-tracker.c ,
https://commits.kde.org/kwindowsystem/b485043e5633b9a07e3cc6804a66ddbe2c134583 ,
https://bugs.kde.org/show_bug.cgi?id=473780 ,
https://bugs.kde.org/show_bug.cgi?id=459986 ,
https://forum.snapcraft.io/t/visibility-of-processes-originating-from-other-snaps/703 .

## 4. Process lineage, sessions and cgroups (Linux, compositor-independent)

What it is. The launcher owns the process it started; the compositor
reports the pid of the window's client; the kernel can say whether the
second descends from the first. Three handles exist: the parent chain
(`/proc/<pid>/status` `PPid`, intact while every intermediate process is
alive), the session id (`/proc/<pid>/stat` field 6, set by `setsid`, which
keyvo's `launch` already calls), and the cgroup (`/proc/<pid>/cgroup`,
unique per transient systemd scope). KIO runs each launch in its own
systemd scope or service (`SystemdProcessRunner`, `ScopedProcessRunner`),
ulauncher wraps the command in `systemd-run --user --scope` when it runs
under systemd, and Flatpak creates `app-flatpak-<appid>-<instanceid>.scope`
with its own pid (`flatpak_run_in_transient_unit`), then `flatpak run`
execs `bwrap`, which forks the application, so the parent chain from
keyvo's shell to the sandboxed process is `sh` (or its exec'd child),
`bwrap`, the app.

Where it breaks. Single-instance handoff: VS Code's `code .`, browsers,
GApplication primaries and `flatpak run` of an already running instance
start a short-lived process that talks to the primary and exits 0; the
window belongs to the old pid, outside keyvo's lineage, session and scope.
The AutoHotkey forum thread on Chrome and LibreOffice's "imperfect" 1 s
`WaitForInputIdle` are the same failure on Windows; GLib's `launched`
signal reports `pid` 0 for D-Bus activated applications because the bus,
not the launcher, started the process. D-Bus activation in general (and
Flatpak's D-Bus activation through the session bus) starts the process
under systemd or dbus-daemon, outside the launcher's scope.

Windows analogue. A job object: children of a process in a job join the
job by default (`CreateProcess`, not WMI), and `IsProcessInJob` answers
whether the foreground window's process belongs to the launch. macOS
analogue: `NSRunningApplication` from the launch completion handler.

Sources:
https://invent.kde.org/frameworks/kio/-/raw/master/src/gui/kprocessrunner.cpp ,
https://raw.githubusercontent.com/Ulauncher/Ulauncher/main/ulauncher/utils/launch_detached.py ,
https://github.com/flatpak/flatpak/raw/main/common/flatpak-run.c ,
https://docs.gtk.org/gio/signal.AppLaunchContext.launched.html ,
https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects ,
https://learn.microsoft.com/en-us/windows/win32/api/jobapi/nf-jobapi-isprocessinjob ,
https://www.autohotkey.com/docs/v2/lib/Run.htm .

## 5. Windows

What it is. `CreateProcess` returns the pid; `ShellExecuteEx` returns a
process handle only with `SEE_MASK_NOCLOSEPROCESS` (LibreOffice's commit
adds the flag and then waits with `WaitForInputIdle(hProcess, 1000)`,
calling the wait "imperfect"). The window is found with `EnumWindows` plus
`GetWindowThreadProcessId`, filtering out hidden and tool windows; the
`startt` crate exists because for Chrome and anything launched through
`cmd /c start`, Explorer or a protocol handler "the PID you get back is
unusable", and it snapshots child processes (Toolhelp32) and ranks
candidate windows by process creation time. Packaged apps are started with
`IApplicationActivationManager::ActivateApplication`, which returns the
app's `processId`. Foreground changes arrive through `SetWinEventHook`
with `EVENT_SYSTEM_FOREGROUND` (hwnd, then pid); a launcher that wants the
new window in front calls `AllowSetForegroundWindow(pid)` first.
AutoHotkey's documented pattern is `Run ..., OutputVarPID` then
`WinWait "ahk_pid " pid`, with the note that the pid is blank for
documents and shell verbs. Logi Options+ Smart Actions have an
"Application" action (open, close, bring to foreground) and the MX
Creative Console a custom "Run" action; the Logi Actions SDK documentation
(C# and Node.js) shows no launch-and-wait API, so a bridge plugin would do
its own Win32 work.

Flatpak and single instance. Not applicable; single-instance handoff is
the main pitfall (Chrome, Office).

Sources:
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-waitforinputidle ,
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-allowsetforegroundwindow ,
https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication ,
https://kod.pardus.org.tr/libreoffice/core/commit/de26ed225eb03ab36efed592774e460b21e695fa ,
https://docs.rs/crate/startt/latest/source/README.md ,
https://www.autohotkey.com/docs/v2/lib/Run.htm ,
https://support.logi.com/hc/articles/14308014808215 ,
https://support.logi.com/hc/en-au/articles/29750730987415-MX-Creative-Console-custom-action-Run ,
https://logitech.github.io/actions-sdk-docs/getting-started .

## 6. macOS

What it is. `NSWorkspace.openApplication(at:configuration:completionHandler:)`
launches asynchronously and hands the completion handler an
`NSRunningApplication` (bundle identifier, pid, activation state) or an
error; by default it does not start a second instance of a running app,
so the handler returns the existing one (`createsNewApplicationInstance`
opts out). `NSWorkspace` posts `didLaunchApplicationNotification` and
`didActivateApplicationNotification`, each with the affected
`NSRunningApplication` under `NSWorkspaceApplicationKey`; the `frontmost`
crate shows the observer pattern for the activation notification.
Hammerspoon's `hs.application.open(app, wait, waitForFirstWindow)` calls
`launchOrFocus`, then polls `mainWindow()` until the timeout, with the
warning that a windowless app never satisfies `waitForFirstWindow`. The
identity on both sides is the bundle identifier, which makes macOS the
cleanest platform: the launch returns the app object, the focus event
carries the same object.

Sources:
https://developer.apple.com/documentation/appkit/nsworkspace/openapplication(at:configuration:completionhandler:) ,
https://developer.apple.com/documentation/appkit/nsrunningapplication ,
https://developer.apple.com/documentation/appkit/nsworkspace/didlaunchapplicationnotification ,
https://developer.apple.com/forums/thread/719474 ,
https://docs.rs/crate/frontmost/1.1.0/source/README.md ,
https://raw.githubusercontent.com/Hammerspoon/hammerspoon/master/extensions/application/application.lua .

## 7. Control surfaces and launchers

Elgato Stream Deck. The built-in "Open" action opens an application, files
or folders; the help article documents no wait and no already-running
behaviour. BarRaider's Advanced Launcher plugin adds "bring to front if
already running" (only with a max-instances setting) and a running
indicator, which shows the gap users feel. None of these feed a
page-switching rule; the Stream Deck app's profile switching is a separate
per-application setting.

StreamController (Linux). The OS plugin's `Launch` action reads `Exec`
from the desktop files under `/usr/share/applications` and
`~/.local/share/applications` and runs `subprocess.Popen([exec],
start_new_session=True)`: no token, no scope, no tracking. Its automatic
page switching on KDE goes through kdotool, which has no `--sync`.

ulauncher. Mints the id through the GDK launch context when the entry has
`StartupNotify`, sets both `DESKTOP_STARTUP_ID` and `XDG_ACTIVATION_TOKEN`
("same as GLib sets"), launches `DBusActivatable` entries through
`gapplication launch`, and wraps the rest in `systemd-run --user --scope`
with `setsid`.

rofi. Uses libstartup-notification on X11 for `Exec` entries and
`g_app_info_launch` with a null context for `DBusActivatable` ones (issue
1924), so no token on Wayland.

GNOME Shell. `shell_app_launch` uses
`g_desktop_app_info_launch_uris_as_manager_with_fds` with a launch
context whose `desktop-startup-id` and `activation-token` are the same
string; app state follows the startup sequences (section 3).

KDE. `KIO::ApplicationLauncherJob` and `CommandLauncherJob` (section 2 and
4): startup id on X11, activation token on Wayland, systemd scope or
service per launch, D-Bus activation when possible.

Scripting tools. `xdotool search --sync --onlyvisible --class <c>` blocks
until a matching window exists (Qubes' tests confirm the window is
visible before typing); `xtoolwait` waits for the first mapped window of
the process; `kdotool` lacks `--sync`.

Sources:
https://help.elgato.com/hc/en-us/articles/360028234471 ,
https://github.com/BarRaider/streamdeck-advancedlauncher ,
https://raw.githubusercontent.com/StreamController/OSPlugin/main/Launch.py ,
https://github.com/StreamController/StreamController ,
https://raw.githubusercontent.com/Ulauncher/Ulauncher/main/ulauncher/modes/apps/launch_app.py ,
https://raw.githubusercontent.com/davatorium/rofi/next/source/modes/drun.c ,
https://github.com/davatorium/rofi/issues/1924 ,
https://gitlab.gnome.org/GNOME/gnome-shell/-/raw/main/src/shell-app.c ,
https://manpages.debian.org/wheezy/xdotool/xdotool.1.en.html ,
https://raw.githubusercontent.com/jinliu/kdotool/master/README.md .

## 8. Comparison

| Mechanism | KDE Wayland | Linux X11 | Windows | macOS | Through `sh -c`? | Flatpak? | Single-instance apps? | Per-key configuration? |
|---|---|---|---|---|---|---|---|---|
| Class or app id match with timeout (pending list) | yes: `resourceClass` is the app id or the sandbox id | yes: WM_CLASS, plus `StartupWMClass` | yes: exe name or AppUserModelID | yes: bundle id | yes | yes | yes, the only one that does | optional `class`; derivable from `flatpak run <id>` or a desktop entry |
| Startup notification id on the window (`_NET_STARTUP_ID`) | XWayland windows only | yes, the window carries the id | no | no | yes (env) | yes (env passes through) | only if the toolkit forwards the id | no |
| xdg-activation token | token not granted to a surfaceless daemon; no feedback to third parties | n/a | n/a | n/a | yes (env) | yes (env or D-Bus `platform_data`) | only through D-Bus activation | no |
| Compositor startup feedback (Plasma activation feedback, mutter sequences) | app id and `finished` for granted tokens only; private protocol | mutter sequences | n/a | n/a | n/a | yes | no | no |
| Process lineage, session or scope against the window's pid | yes: KWin reports the host pid | yes with XRES (not `_NET_WM_PID`) | job object + `IsProcessInJob` | `NSRunningApplication` from the launch | yes | yes: host pid, lineage through `bwrap`, `.flatpak-info`, scope name | no | no |
| Wait for a window of the pid (`WaitForInputIdle`, `xdotool --sync`, `hs.application.open`) | kdotool has no `--sync` | `xdotool search --sync` | yes | yes | yes | pid lineage needed | no | no |
| Native app object from the launch API | n/a | n/a | `ActivateApplication` pid (packaged apps) | `openApplication` completion | no (needs the API, not a shell) | n/a | yes on macOS (returns the running instance) | no |

## 9. What this changes for keyvo

The field has no cross-platform "which window is mine" primitive. Every
desktop shell ends up matching the application identity it already shows
in its task bar (app id, desktop file, WM class, bundle id) against the
launch, with a timeout: Plasma's task manager hides a startup when a
window with the same `AppId` or `AppName` appears and drops it after 5 s,
KWin itself accepts a window whose `desktopFileName` equals the app id of
the last granted token, GNOME Shell's tracker matches WM class and
sandbox id before anything launch-specific. The pending list with a class
match is therefore the right primary design, not a fallback. What the
prior art adds is a second, stronger signal that is cheap on Linux and
has native equivalents elsewhere, and a clear verdict on tokens.

Recommended layers, in the order the app switch is judged:

1. Pid lineage first, when the window watcher reports a pid (KWin does,
   the X11 watcher should use XRES, the Windows bridge gets it from the
   foreground hwnd). A pending launch records its shell pid; the daemon
   accepts the window when `/proc/<pid>/stat` reports the launch's session
   id (keyvo already starts launches with `setsid`, so the session id is
   the shell's pid), or when the `PPid` chain reaches the launch, or when
   `/proc/<pid>/cgroup` names a transient scope keyvo created for that
   launch (`systemd-run --user --scope --unit keyvo-launch-<n>`, the KIO
   and ulauncher pattern). For Flatpak the chain is intact (`flatpak`
   execs `bwrap`, which forks the app) and `/proc/<pid>/root/.flatpak-info`
   gives the app id and instance id on top. This decides between two
   launches of the same class in flight and never misattributes a mouse
   switch to a launch. On Windows the same layer is a job object per
   launch and `IsProcessInJob`; on macOS it is the `NSRunningApplication`
   the launch returns, compared by pid or bundle id.
2. Class match second: the explicit `class` on the key as today, and
   otherwise a class derived from the command (`flatpak run <id>` gives
   the app id, which is KWin's `resourceClass` for that window; a desktop
   entry gives `StartupWMClass` or its file name). This is the layer that
   covers single-instance handoff, where lineage breaks by design, and
   D-Bus activated applications, whose process the bus starts.
3. Oldest unmatched pending launch last, as planned, and foreign
   otherwise.

Consequences for the spec:

- `[timing].launch_focus` should outlive Flatpak cold starts; Plasma
  uses 5 s for its startup feedback, mutter 15 s, the startup notification
  spec asks launchers for at least a minute. 15 s is a defensible default
  for the owner's 4 to 8 s cold starts; 10 s is the lower bound.
- A `launch` that exits 0 within `launch_settle` (the "already running
  hands over" case) must still leave its pending entry alive, because the
  app switch comes from the primary instance a moment later; only a
  non-zero exit or a spawn failure drops the entry at once.
- The window watcher's event should carry what it already has from KWin:
  `resourceClass`, caption and pid; `desktopFileName` adds nothing on
  Wayland (same value unless a window rule overrides it). The X11 watcher
  should read the pid through XRES and may read `_NET_STARTUP_ID` as a
  free extra match when keyvo sets `DESKTOP_STARTUP_ID`.
- xdg-activation tokens are not a correlation tool for keyvo on Wayland:
  KWin grants none to a daemon without an active surface or a fresh input
  serial, mutter creates a sequence but does not tag the window, and the
  protocol has no "who used it" event. Setting `XDG_ACTIVATION_TOKEN` and
  `DESKTOP_STARTUP_ID` in the child's environment is harmless and the
  variables pass through `flatpak run`, but on KDE the only thing a token
  could buy is focus under a stricter focus stealing prevention level,
  and that needs a granted token, which keyvo cannot obtain.
- Focus stealing prevention is the hidden dependency of the whole feature:
  at the default level (Low) KWin activates every new window, which is
  why launches switch the pad today; at Medium or above a window launched
  by keyvo would only demand attention, so no app switch arrives and the
  pad stays where it is. The hardware notes should record the owner's
  level (default, KWin 6.7.4), and the doctor command could warn when the
  level is above Low.

Open questions:

- Does `bwrap` (or `flatpak run`) call `setsid` on the owner's version, so
  that the session id check fails for Flatpak and only the parent chain or
  the scope remains? To be measured on the host with
  `cat /proc/<pid>/stat` for a launched Flatpak. Answered on 2026-10-28:
  neither does; KCalc's session id equals the pid of the top `flatpak
  run` process (`docs/hardware-notes.md`, "Launch focus on KWin and
  Flatpak").
- Should keyvo run launches in a transient systemd scope at all (it also
  fixes the `KillMode=control-group` note of §8 for a future unit), or is
  the `setsid` session id enough for the pre-alpha?
- Windows bridge: whether the Logi Actions SDK lets a plugin observe the
  foreground window at all, or whether the bridge must install its own
  `SetWinEventHook`; and whether a job object survives the SDK host's
  process model.
- Deriving the class from a command line: how far to go (Flatpak id and
  desktop entry are safe; a bare executable name is a guess that the
  explicit `class` field should override).
- Whether to surface a "launched, waiting for window" look on the key
  while a pending launch is alive, the way Plasma shows the bouncing
  cursor; this is a UX question for the owner, not a matching one.
