<!-- Notifications.svelte -->
<script>
  import { notifications } from "../stores/explorer.js";
  const ICONS = { success: "✓", error: "✕", info: "ℹ", warning: "⚠" };
</script>

<div class="notif-stack">
  {#each $notifications as n (n.id)}
    <div class="notif notif-{n.type}" role="alert">
      <span class="notif-icon">{ICONS[n.type] || "ℹ"}</span>
      <span class="notif-msg">{n.message}</span>
    </div>
  {/each}
</div>

<style>
  .notif-stack {
    position: fixed;
    bottom: 40px;
    right: 16px;
    z-index: 300;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-end;
  }
  .notif {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-radius: var(--radius);
    
    border: 1px solid var(--border);
    background: var(--bg-primary);
    font-size: 12px;
    max-width: 320px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.3);
    animation: slide-in 0.18s ease;
  }
  .notif-success { border-color: rgba(90,255,142,0.3); }
  .notif-error   { border-color: rgba(255,95,109,0.3); }
  .notif-warning { border-color: rgba(255,209,102,0.3); }
  .notif-icon { font-size: 13px; }
  .notif-success .notif-icon { color: var(--success); }
  .notif-error   .notif-icon { color: var(--danger); }
  .notif-warning .notif-icon { color: var(--warning); }
  @keyframes slide-in {
    from { transform: translateX(20px); opacity: 0; }
    to   { transform: translateX(0);    opacity: 1; }
  }
</style>
