<!-- StatusBar.svelte -->
<script>
  import {
    displayEntries, selectedPaths, currentPath, searchQuery,
    gitStatus, activePaneId, adminMode,
    rightPanePath, rightDisplayCount, rightTotalSize, rightSelectedCount,
  } from "../stores/explorer.js";

  function formatSize(bytes) {
    if (!bytes) return "0 B";
    const u = ["B","KB","MB","GB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < u.length-1) { n /= 1024; i++; }
    return `${n.toFixed(i?1:0)} ${u[i]}`;
  }

  $: totalSize    = $displayEntries.reduce((s, e) => s + e.size, 0);
  $: selectedCount = $selectedPaths.size;

  // Aktif pane'e göre değerleri seç
  $: activeCount    = $activePaneId === "right" ? $rightDisplayCount  : $displayEntries.length;
  $: activeSelCount = $activePaneId === "right" ? $rightSelectedCount : selectedCount;
  $: activeSize     = $activePaneId === "right" ? $rightTotalSize     : totalSize;
  $: activePath     = $activePaneId === "right" ? $rightPanePath      : $currentPath;
</script>

<div class="statusbar">
  <span class="stat">{activeCount} items</span>
  {#if activeSelCount > 0}
    <span class="stat selected">{activeSelCount} selected</span>
  {/if}
  {#if $searchQuery && $activePaneId === "left"}
    <span class="stat search">Search: "{$searchQuery}"</span>
  {/if}
  {#if $gitStatus?.is_repo && $activePaneId === "left"}
    <span class="stat git-branch">⎇ {$gitStatus.branch}</span>
    {#if $gitStatus.counts?.modified > 0}
      <span class="stat git-m">{$gitStatus.counts.modified}M</span>
    {/if}
    {#if $gitStatus.counts?.added > 0}
      <span class="stat git-a">{$gitStatus.counts.added}A</span>
    {/if}
    {#if $gitStatus.counts?.untracked > 0}
      <span class="stat git-u">{$gitStatus.counts.untracked}?</span>
    {/if}
  {/if}
  {#if $adminMode}
    <span class="stat admin-badge" title="Yönetici (Admin) Modu Aktif — Tüm işlemler root yetkisiyle yapılır">⚡ ADMIN</span>
  {/if}
  <span class="spacer"></span>
  <span class="stat dim">{formatSize(activeSize)}</span>
  <span class="stat path">{activePath}</span>
</div>

<style>
  .statusbar {
    height: var(--statusbar-height);
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 10px;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    font-size: 11px;
    color: var(--text-dim);
  }
  .stat { white-space: nowrap; }
  .stat.selected { color: var(--accent); }
  .stat.search { color: var(--warning); }
  .stat.path { font-family: var(--font-mono); font-size: 10px; opacity: 0.6; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .stat.git-branch { color: var(--accent); font-family: var(--font-mono); font-size: 10px; }
  .stat.git-m { color: #ffd166; font-size: 10px; font-weight: 600; }
  .stat.git-a { color: #5aff8e; font-size: 10px; font-weight: 600; }
  .stat.git-u { color: #8898aa; font-size: 10px; font-weight: 600; }
  .stat.admin-badge {
    background: rgba(255, 75, 75, 0.2);
    color: #ff5555;
    border: 1px solid rgba(255, 75, 75, 0.4);
    border-radius: 4px;
    padding: 1px 6px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
  }
  .spacer { flex: 1; }
</style>
