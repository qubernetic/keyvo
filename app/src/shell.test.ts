// The Tauri shell's security surface: the window may call the app's own
// commands and listen to events, nothing else, and the page may load only
// its own code (SECURITY.md, "The desktop app").
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const APP = join(dirname(fileURLToPath(import.meta.url)), "..");
const SHELL = join(APP, "src-tauri");

function read(path: string): string {
  return readFileSync(path, "utf8");
}

/** The commands `generate_handler!` registers in the shell's main.rs. */
function registered(): string[] {
  const handler = /generate_handler!\[([^\]]*)\]/.exec(read(join(SHELL, "src/main.rs")));
  expect(handler, "no generate_handler! in main.rs").not.toBeNull();
  return (handler?.[1] ?? "")
    .split(",")
    .map((name) => name.trim())
    .filter((name) => name !== "");
}

/** The commands build.rs puts into the app manifest. */
function manifest(): string[] {
  const list = /AppManifest::new\(\)\s*\.commands\(&\[([^\]]*)\]\)/.exec(
    read(join(SHELL, "build.rs")),
  );
  expect(list, "no AppManifest commands in build.rs").not.toBeNull();
  return [...(list?.[1] ?? "").matchAll(/"([a-z_]+)"/g)].map((match) => match[1] ?? "");
}

/** The commands the frontend invokes. */
function invoked(): string[] {
  const lib = join(APP, "src/lib");
  const names = readdirSync(lib)
    .filter((file) => file.endsWith(".ts") && !file.endsWith(".test.ts"))
    .flatMap((file) => [...read(join(lib, file)).matchAll(/invoke(?:<[^>]*>)?\("([a-z_]+)"/g)])
    .map((match) => match[1] ?? "");
  return [...new Set(names)].sort();
}

describe("the main window's capability", () => {
  const capability = JSON.parse(read(join(SHELL, "capabilities/default.json"))) as {
    windows: string[];
    permissions: string[];
  };
  const commands = registered();

  it("lists the app manifest's commands exactly as main.rs registers them", () => {
    expect(manifest()).toEqual(commands);
  });

  it("allows each app command and the event listener, nothing else", () => {
    expect(capability.windows).toEqual(["main"]);
    expect(capability.permissions).toEqual([
      ...commands.map((command) => `allow-${command.replaceAll("_", "-")}`),
      "core:event:allow-listen",
      "core:event:allow-unlisten",
    ]);
    expect(capability.permissions).not.toContain("core:default");
  });

  it("covers every command the frontend invokes", () => {
    expect(invoked().every((command) => commands.includes(command))).toBe(true);
    expect(invoked()).toEqual([...commands].sort());
  });
});

describe("the content security policy", () => {
  const config = JSON.parse(read(join(SHELL, "tauri.conf.json"))) as {
    app: { security: { csp: string } };
  };

  it("loads only the app's own code and talks only to the IPC", () => {
    expect(config.app.security.csp).toBe(
      "default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:",
    );
  });
});
