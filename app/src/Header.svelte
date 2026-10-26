<script lang="ts">
  // The header (docs/design/gui-ux.md §2, d8, d13; §5.2): the app name,
  // the device selector (every device the daemon reports plus the
  // Simulator; the Simulator while the Keypad is missing) and the overflow
  // menu, whose entries stay disabled until they exist.
  import Icon from "./Icon.svelte";
  import { t, type MessageKey } from "./lib/i18n";
  import type { DeviceStatus } from "./lib/protocol";
  import {
    chosenDevice,
    deviceChoices,
    MENU,
    type DeviceChoice,
    type MenuId,
  } from "./lib/shell";

  let {
    device,
    chosen,
    onchoose,
    onshortcuts,
  }: {
    /** What the daemon reports; none while it cannot be reached. */
    device?: DeviceStatus;
    /** The user's last choice, if any. */
    chosen?: DeviceChoice;
    onchoose: (choice: DeviceChoice) => void;
    /** Opens the keyboard map, the menu's "Keyboard shortcuts". */
    onshortcuts?: () => void;
  } = $props();

  const ACTIONS: Partial<Record<MenuId, () => void>> = {
    shortcuts: () => onshortcuts?.(),
  };

  function choose(id: MenuId) {
    const action = ACTIONS[id];
    if (action === undefined) return;
    close(false);
    action();
  }

  const NAMES: Record<DeviceChoice, MessageKey> = {
    keypad: "header.keypad",
    simulator: "header.simulator",
  };

  const choices = $derived(deviceChoices(device));
  const shown = $derived(chosenDevice(device, chosen));

  let open = $state(false);
  let toggle: HTMLButtonElement | undefined = $state();
  let menu: HTMLElement | undefined = $state();
  const items: HTMLButtonElement[] = $state([]);

  async function show() {
    open = true;
    await Promise.resolve();
    items[0]?.focus();
  }

  function close(refocus: boolean) {
    open = false;
    if (refocus) toggle?.focus();
  }

  function menuKeys(event: KeyboardEvent) {
    const at = items.findIndex((item) => item === document.activeElement);
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (step !== undefined) {
      event.preventDefault();
      items[(at + step + items.length) % items.length]?.focus();
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      items[event.key === "Home" ? 0 : items.length - 1]?.focus();
    } else if (event.key === "Escape") {
      event.preventDefault();
      close(true);
    } else if (event.key === "Tab") {
      close(false);
    }
  }

  function outside(event: MouseEvent) {
    const target = event.target;
    if (!open || !(target instanceof Node)) return;
    if (menu?.contains(target) === true || toggle?.contains(target) === true) return;
    close(false);
  }
</script>

<svelte:window onclick={outside} />

<header class="header">
  <h1>{t("app.name")}</h1>
  <select
    aria-label={t("header.device")}
    value={shown}
    onchange={(event) => onchoose(event.currentTarget.value as DeviceChoice)}
  >
    {#each choices as choice (choice)}
      <option value={choice}>{t(NAMES[choice])}</option>
    {/each}
  </select>
  <div class="overflow">
    <button
      bind:this={toggle}
      type="button"
      class="toggle"
      aria-label={t("header.menu")}
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls="kv-menu"
      onclick={() => (open ? close(false) : void show())}
    >
      <Icon name="ellipsis-vertical" />
    </button>
    {#if open}
      <ul
        bind:this={menu}
        id="kv-menu"
        role="menu"
        tabindex="-1"
        aria-label={t("header.menu")}
        onkeydown={menuKeys}
      >
        {#each MENU as entry, index (entry.id)}
          <li role="none">
            <button
              bind:this={items[index]}
              type="button"
              role="menuitem"
              tabindex="-1"
              aria-label={t(entry.label)}
              aria-disabled={entry.enabled ? undefined : "true"}
              aria-describedby={entry.enabled ? undefined : `kv-menu-soon-${index}`}
              onclick={() => {
                if (entry.enabled) choose(entry.id);
              }}
            >
              <span>{t(entry.label)}</span>
              {#if !entry.enabled}
                <span id={`kv-menu-soon-${index}`} class="soon">{t("soon")}</span>
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 16px;
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
  }

  h1 {
    margin: 0 auto 0 0;
    font-size: 1.2em;
  }

  .overflow {
    position: relative;
  }

  .toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 32px;
  }

  [role="menu"] {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 1;
    width: max-content;
  }

  .soon {
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }
</style>
