<script>
  import { createEventDispatcher } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { currentPath } from "../stores/explorer.js";

  const dispatch = createEventDispatcher();
  const win = getCurrentWindow();

  let maximized = false;

  async function toggleMax() {
    maximized = await win.isMaximized();
    if (maximized) { await win.unmaximize(); maximized = false; }
    else           { await win.maximize();   maximized = true;  }
  }
</script>

<div class="titlebar" data-tauri-drag-region>
  <!-- Left: App icon + name -->
  <div class="titlebar-left">
    <span class="app-icon">◈</span>
    <span class="app-name">Arch Explorer</span>
  </div>

  <!-- Center: action buttons -->
  <div class="titlebar-center">
    <button class="tb-btn" title="Disks & Mounts" on:click={() => dispatch("mounts")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <ellipse cx="12" cy="6" rx="10" ry="4"/>
        <path d="M2 6v6c0 2.21 4.48 4 10 4s10-1.79 10-4V6"/>
        <path d="M2 12v6c0 2.21 4.48 4 10 4s10-1.79 10-4v-6"/>
      </svg>
      Disks
    </button>
    <button class="tb-btn" title="Ağ Paylaşımları" on:click={() => dispatch("network")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="5" r="3"/><circle cx="5" cy="19" r="3"/><circle cx="19" cy="19" r="3"/>
        <path d="M12 8v4M12 12l-5 4M12 12l5 4"/>
      </svg>
      Network
    </button>
    <button class="tb-btn" title="Dual Pane (Ctrl+P)" on:click={() => dispatch("dual-pane")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="2" y="3" width="9" height="18" rx="1"/><rect x="13" y="3" width="9" height="18" rx="1"/>
      </svg>
      Panes
    </button>
    <button class="tb-btn" title="Plugins" on:click={() => dispatch("plugins")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 2l2 4h4l-3 3 1 4-4-2-4 2 1-4-3-3h4z"/>
      </svg>
      Plugins
    </button>
    <button class="tb-btn" title="Commands" on:click={() => dispatch("commands")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
      </svg>
      Commands
    </button>
    <button class="tb-btn" title="Settings" on:click={() => dispatch("settings")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"/>
        <path d="M12 2v2M12 20v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M2 12h2M20 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
      </svg>
      Settings
    </button>
  </div>

  <!-- Right: window controls -->
  <div class="titlebar-right">
    <button class="wc-btn minimize" on:click={() => win.minimize()} title="Minimize">
      <span>—</span>
    </button>
    <button class="wc-btn maximize" on:click={toggleMax} title="Maximize">
      <span>{maximized ? "❐" : "□"}</span>
    </button>
    <button class="wc-btn close" on:click={() => win.close()} title="Close">
      <span>✕</span>
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: var(--toolbar-height);
    display: flex;
    align-items: center;
    padding: 0 12px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 8px;
  }

  .titlebar-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 140px;
  }

  .app-icon {
    font-size: 18px;
    color: var(--accent);
    line-height: 1;
  }

  .app-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: 0.02em;
  }

  .titlebar-center {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    justify-content: center;
  }

  .tb-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    font-size: var(--font-size-sm);
    cursor: pointer;
    transition: var(--transition);
  }
  .tb-btn:hover {
    background: var(--bg-hover);
    border-color: var(--border);
    color: var(--text-primary);
  }

  .titlebar-right {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 140px;
    justify-content: flex-end;
  }

  .wc-btn {
    width: 30px;
    height: 26px;
    border-radius: var(--radius-sm);
    border: none;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    transition: var(--transition-fast);
  }
  .wc-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .wc-btn.close:hover { background: var(--danger-soft); color: var(--danger); }
</style>
