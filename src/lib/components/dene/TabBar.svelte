<script>
  import {
    tabs, activeTabId, addTab, closeTab, switchTab, tabBarMode,
    showDualPane, activePaneId,
    rightPaneTabs, rightActiveTabId, rightPaneNavRequest,
  } from "../stores/explorer.js";

  let dragSrcId = null;

  // lazy modda: tek tabsa ve dual pane kapalıysa gizle
  $: visible = $tabBarMode === "always"
    || $tabs.length > 1
    || ($showDualPane && $rightPaneTabs.length > 0);

  // ── Sol pane drag ──
  function onDragStart(e, id) { dragSrcId = id; e.dataTransfer.effectAllowed = "move"; }

  function onDrop(e, targetId) {
    e.preventDefault();
    if (dragSrcId === null || dragSrcId === targetId) return;
    tabs.update(ts => {
      const srcIdx  = ts.findIndex(t => t.id === dragSrcId);
      const dstIdx  = ts.findIndex(t => t.id === targetId);
      const arr     = [...ts];
      const [moved] = arr.splice(srcIdx, 1);
      arr.splice(dstIdx, 0, moved);
      return arr;
    });
    dragSrcId = null;
  }

  function onMiddleClick(e, id) {
    if (e.button === 1) { e.preventDefault(); closeTab(id); }
  }

  // ── Sağ pane tab işlemleri ──
  function rightSwitchTab(id) {
    rightPaneNavRequest.set({ type: "switch-tab", tabId: id });
    activePaneId.set("right");
  }

  function rightCloseTab(id) {
    rightPaneNavRequest.set({ type: "close-tab", tabId: id });
  }

  function rightNewTab() {
    rightPaneNavRequest.set({ type: "new-tab" });
    activePaneId.set("right");
  }

  // Ctrl+T yeni tab (aktif pane'e), Ctrl+W kapat
  function onKeyDown(e) {
    if (e.ctrlKey && e.key === "t") {
      e.preventDefault();
      if ($activePaneId === "right") rightNewTab(); else addTab();
    }
    if (e.ctrlKey && e.key === "w") {
      e.preventDefault();
      if ($activePaneId === "right") rightCloseTab($rightActiveTabId); else closeTab($activeTabId);
    }
    if (e.ctrlKey && e.key === "Tab") {
      e.preventDefault();
      const ts  = $tabs;
      const idx = ts.findIndex(t => t.id === $activeTabId);
      const next = e.shiftKey
        ? ts[(idx - 1 + ts.length) % ts.length]
        : ts[(idx + 1) % ts.length];
      switchTab(next.id);
    }
  }
</script>

<svelte:window on:keydown={onKeyDown} />

{#if visible}
<div class="tabbar">

  {#if $showDualPane}
    <!-- ── Dual pane: sol yarı | ayraç | sağ yarı ── -->
    <!-- Sol pane tabs -->
    <div class="tab-half" class:half-active={$activePaneId === "left"}
      on:mousedown|stopPropagation={() => activePaneId.set("left")}>
      <div class="tab-list">
        {#each $tabs as tab (tab.id)}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="tab"
            class:active={tab.id === $activeTabId && $activePaneId === "left"}
            draggable="true"
            on:click={() => { switchTab(tab.id); activePaneId.set("left"); }}
            on:mousedown={e => onMiddleClick(e, tab.id)}
            on:dragstart={e => onDragStart(e, tab.id)}
            on:dragover|preventDefault
            on:drop={e => onDrop(e, tab.id)}
            title={tab.path}
          >
            <span class="tab-icon">◈</span>
            <span class="tab-label">{tab.label}</span>
            {#if $tabs.length > 1}
              <span class="tab-close" on:click|stopPropagation={() => closeTab(tab.id)}>✕</span>
            {/if}
          </div>
        {/each}
      </div>
      <button class="new-tab-btn" on:click={() => { addTab(); activePaneId.set("left"); }}>+</button>
    </div>

    <!-- Ortadaki ayraç -->
    <div class="tab-divider"></div>

    <!-- Sağ pane tabs -->
    <div class="tab-half right-half" class:half-active={$activePaneId === "right"}
      on:mousedown|stopPropagation={() => activePaneId.set("right")}>
      <div class="tab-list">
        {#each $rightPaneTabs as tab (tab.id)}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="tab right-tab"
            class:active={tab.id === $rightActiveTabId && $activePaneId === "right"}
            on:click={() => rightSwitchTab(tab.id)}
            on:mousedown={e => { if (e.button === 1) rightCloseTab(tab.id); }}
            title={tab.path}
          >
            <span class="tab-icon" style="color:var(--warning)">◈</span>
            <span class="tab-label">{tab.label}</span>
            {#if $rightPaneTabs.length > 1}
              <span class="tab-close" on:click|stopPropagation={() => rightCloseTab(tab.id)}>✕</span>
            {/if}
          </div>
        {/each}
      </div>
      <button class="new-tab-btn" on:click={rightNewTab}>+</button>
    </div>

  {:else}
    <!-- ── Tekli mod: mevcut hali ── -->
    <div class="tab-list">
      {#each $tabs as tab (tab.id)}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="tab"
          class:active={tab.id === $activeTabId}
          draggable="true"
          on:click={() => switchTab(tab.id)}
          on:mousedown={e => onMiddleClick(e, tab.id)}
          on:dragstart={e => onDragStart(e, tab.id)}
          on:dragover|preventDefault
          on:drop={e => onDrop(e, tab.id)}
          title={tab.path}
        >
          <span class="tab-icon">◈</span>
          <span class="tab-label">{tab.label}</span>
          {#if $tabs.length > 1}
            <span class="tab-close" on:click|stopPropagation={() => closeTab(tab.id)}>✕</span>
          {/if}
        </div>
      {/each}
    </div>
    <button class="new-tab-btn" on:click={() => addTab()}>+</button>
  {/if}

</div>
{/if}

<style>
  .tabbar {
    display: flex;
    align-items: stretch;
    height: 34px;
    background: var(--bg-primary);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    overflow: hidden;
    gap: 0;
  }

  /* Dual pane yarı-panel */
  .tab-half {
    display: flex;
    align-items: stretch;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    opacity: 0.65;
  }
  .tab-half.half-active { opacity: 1; }
  .tab-half.right-half { border-left: none; }

  /* Sol/sağ ayraç */
  .tab-divider {
    width: 4px;
    background: var(--border);
    flex-shrink: 0;
  }

  .tab-list {
    display: flex;
    align-items: stretch;
    flex: 1;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }
  .tab-list::-webkit-scrollbar { display: none; }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px 0 10px;
    min-width: 100px;
    max-width: 200px;
    border-right: 1px solid var(--border);
    cursor: pointer;
    font-size: 11px;
    color: var(--text-dim);
    background: var(--bg-primary);
    transition: background 0.1s, color 0.1s;
    user-select: none;
    position: relative;
  }
  .tab:hover { background: var(--bg-hover); color: var(--text-secondary); }
  .tab.active {
    background: var(--bg-secondary);
    color: var(--text-primary);
    border-bottom: 2px solid var(--accent);
  }

  .tab-icon {
    font-size: 10px;
    color: var(--accent);
    flex-shrink: 0;
  }
  .tab-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .tab-close {
    flex-shrink: 0;
    font-size: 9px;
    color: var(--text-dim);
    padding: 1px 3px;
    border-radius: 3px;
    opacity: 0;
    transition: opacity 0.1s, background 0.1s;
  }
  .tab:hover .tab-close { opacity: 1; }
  .tab-close:hover { background: var(--danger-soft); color: var(--danger); }

  .new-tab-btn {
    width: 34px;
    flex-shrink: 0;
    background: none;
    border: none;
    border-left: 1px solid var(--border);
    color: var(--text-dim);
    font-size: 18px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }
  .new-tab-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
</style>
