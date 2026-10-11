<script>
  import { createEventDispatcher, onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { notify, navigateTo } from "../stores/explorer.js";

  const dispatch = createEventDispatcher();

  export let targetDevice = null;

  let devices     = [];
  let loading     = false;
  let mounting    = null;
  let listEl;
  let blinkDevice = null;
  let activeTarget = targetDevice;

  $: if (targetDevice) {
    activeTarget = targetDevice;
    scrollToTarget(targetDevice);
  }

  function scrollToTarget(devName) {
    const dev = devName || activeTarget;
    if (!dev || !listEl) return;
    const attempt = () => {
      if (!listEl) return false;
      const el = listEl.querySelector(`[data-device="${dev}"]`);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
        blinkDevice = dev;
        setTimeout(() => { if (blinkDevice === dev) blinkDevice = null; }, 1800);
        return true;
      }
      return false;
    };
    if (!attempt()) {
      setTimeout(attempt, 60);
      setTimeout(attempt, 200);
    }
  }

  // Password dialog state
  let showPasswordDialog = false;
  let passwordTarget = null; // { device, mode: 'mount' | 'unmount' }
  let passwordInput = "";
  let passwordError = "";
  let passwordBusy = false;
  let passwordInputEl;

  onMount(loadDevices);

  async function loadDevices(preserveScroll = false) {
    const scrollTop = preserveScroll ? (listEl?.scrollTop ?? 0) : 0;
    loading = true;
    try {
      devices = await invoke("list_block_devices");
    } catch (e) {
      notify("error", String(e));
    } finally {
      loading = false;
      await tick();
      if (preserveScroll && listEl) {
        listEl.scrollTop = scrollTop;
      }
      if (activeTarget) {
        scrollToTarget(activeTarget);
      }
    }
  }

  async function mount(dev) {
    activeTarget = dev.device;
    mounting = dev.device;
    try {
      let result;
      if (dev.is_windows) {
        result = await invoke("mount_ntfs_windows_disk", {
          device: dev.device,
          label: dev.label || null,
          mountPoint: null,
        });
      } else {
        result = await invoke("mount_device", {
          device: dev.device,
          label: dev.label || null,
        });
      }

      // Rust her zaman Ok(MountResult) döndürür — message'a biz bakıyoruz
      const msg = result.message || "";

      if (msg.startsWith("PASSWORD_REQUIRED")) {
        openPasswordDialog(dev, "mount");
      } else if (msg === "HIBERNATION_WARNING") {
        notify("error", "Windows hızlı başlatma aktif! Windows'ta 'shutdown /s /t 0' çalıştır, sonra tekrar dene.");
      } else if (result.success) {
        notify("success", msg || "Mounted!");
        if (result.mount_point) navigateTo(result.mount_point);
        await loadDevices(true);
        scrollToTarget(dev.device);
      } else {
        notify("error", msg || "Mount başarısız");
      }
    } catch (e) {
      notify("error", String(e));
    } finally {
      mounting = null;
    }
  }

  async function unmount(dev) {
    activeTarget = dev.device;
    mounting = dev.device;
    try {
      const msg = await invoke("unmount_device", { device: dev.device });
      notify("success", msg || "Unmounted");
      await loadDevices(true);
      scrollToTarget(dev.device);
    } catch (e) {
      const msg = String(e);
      const msgLow = msg.toLowerCase();
      const needsSudo =
        msg.startsWith("PASSWORD_REQUIRED") ||
        msgLow.includes("password_required") ||
        msgLow.includes("not authorized") ||
        msgLow.includes("superuser") ||
        msgLow.includes("operation not permitted") ||
        msgLow.includes("permission denied") ||
        msgLow.includes("access denied");

      if (needsSudo) {
        openPasswordDialog(dev, "unmount");
      } else {
        notify("error", msg);
      }
    } finally {
      mounting = null;
    }
  }

  function openPasswordDialog(dev, mode) {
    passwordTarget = { device: dev.device, label: dev.label || "", mode };
    passwordInput = "";
    passwordError = "";
    showPasswordDialog = true;
    setTimeout(() => passwordInputEl?.focus(), 50);
  }

  function closePasswordDialog() {
    showPasswordDialog = false;
    passwordTarget = null;
    passwordInput = "";
    passwordError = "";
  }

  async function submitPassword() {
    if (!passwordInput) return;
    passwordBusy = true;
    passwordError = "";
    mounting = passwordTarget.device;

    try {
      if (passwordTarget.mode === "mount") {
        const result = await invoke("mount_ntfs_with_password", {
          device: passwordTarget.device,
          password: passwordInput,
          label: passwordTarget.label || null,
          mountPoint: null,
        });
        notify("success", result.message || "Mounted!");
        if (result.mount_point) navigateTo(result.mount_point);
      } else {
        const msg = await invoke("unmount_with_password", {
          device: passwordTarget.device,
          password: passwordInput,
        });
        notify("success", msg || "Unmounted");
      }
      closePasswordDialog();
      await loadDevices(true);
      if (activeTarget) {
        scrollToTarget(activeTarget);
      }
    } catch (e) {
      const msg = String(e);
      if (msg.includes("WRONG_PASSWORD") || msg.toLowerCase().includes("incorrect")) {
        passwordError = "Yanlış şifre, tekrar dene";
        passwordInput = "";
        setTimeout(() => passwordInputEl?.focus(), 50);
      } else {
        passwordError = msg;
      }
    } finally {
      passwordBusy = false;
      mounting = null;
    }
  }

  function open(dev) {
    if (dev.mount_point) navigateTo(dev.mount_point);
    dispatch("close");
  }
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">
    <div class="panel-header">
      <span>Disks & Mounts</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    {#if loading && devices.length === 0}
      <div class="loading">Scanning devices...</div>
    {:else}
      <div class="device-list" bind:this={listEl}>
        {#each devices as dev}
          <div class="device-row" class:mounted={dev.is_mounted} class:blink={blinkDevice === dev.device} data-device={dev.device}>
            <div class="dev-info">
              <span class="dev-icon">{dev.is_windows ? "🪟" : dev.is_mounted ? "💿" : "⬡"}</span>
              <div class="dev-meta">
                <span class="dev-name">{dev.label || dev.name}</span>
                <span class="dev-detail">{dev.device} · {dev.fs_type} · {dev.size}</span>
                {#if dev.mount_point}
                  <span class="dev-mount">{dev.mount_point}</span>
                {/if}
                {#if dev.is_windows}
                  <span class="win-badge">Windows NTFS</span>
                {/if}
              </div>
            </div>
            <div class="dev-actions">
              {#if dev.is_mounted}
                <button class="btn" on:click={() => open(dev)}>Open</button>
                <button class="btn btn-danger" on:click={() => unmount(dev)}
                  disabled={mounting === dev.device}>
                  {mounting === dev.device ? "..." : "Unmount"}
                </button>
              {:else}
                <button class="btn btn-accent" on:click={() => mount(dev)}
                  disabled={mounting === dev.device}>
                  {mounting === dev.device ? "Mounting..." : dev.is_windows ? "Mount (NTFS Fix)" : "Mount"}
                </button>
              {/if}
            </div>
          </div>
        {/each}

        {#if devices.length === 0}
          <div class="empty">No block devices found</div>
        {/if}
      </div>

      <div class="tip">
        <strong>Windows disk not working?</strong> Polkit agent yoksa şifre soracağız,
        <code>sudo -S</code> ile direkt mount ediyoruz. Otomatik olarak
        kernel <code>ntfs3</code> sonra <code>ntfs-3g</code> deniyoruz.
      </div>
    {/if}

    <div class="panel-footer">
      <button class="btn" on:click={loadDevices}>↻ Refresh</button>
    </div>
  </div>
</div>

<!-- ── Password Dialog ── -->
{#if showPasswordDialog}
  <div class="pw-overlay" on:click|self={closePasswordDialog}>
    <div class="pw-dialog glass">
      <div class="pw-icon">🔒</div>
      <div class="pw-title">Sudo şifresi gerekli</div>
      <div class="pw-sub">
        Polkit agent bulunamadı, <code>{passwordTarget?.device}</code>
        {passwordTarget?.mode === "mount" ? "mount" : "unmount"} etmek için şifren lazım.
      </div>

      <input
        type="password"
        class="pw-input"
        placeholder="Sudo password"
        bind:value={passwordInput}
        bind:this={passwordInputEl}
        disabled={passwordBusy}
        on:keydown={e => e.key === "Enter" && submitPassword()}
      />

      {#if passwordError}
        <div class="pw-error">{passwordError}</div>
      {/if}

      <div class="pw-actions">
        <button class="btn" on:click={closePasswordDialog} disabled={passwordBusy}>Cancel</button>
        <button class="btn btn-accent" on:click={submitPassword} disabled={passwordBusy || !passwordInput}>
          {passwordBusy ? "Mounting..." : "Confirm"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.45);
    
    
    display: flex; align-items: center; justify-content: center;
    z-index: 100;
  }
  .panel {
    width: 560px; max-height: 70vh;
    border-radius: var(--radius-lg);
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
  }
  .panel-header button {
    background: none; border: none;
    color: var(--text-dim); cursor: pointer; font-size: 14px;
  }
  .device-list { flex: 1; overflow-y: auto; padding: 10px; display: flex; flex-direction: column; gap: 6px; }
  .device-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    gap: 12px;
    transition: background 0.2s, border-color 0.2s, box-shadow 0.2s;
  }
  .device-row.mounted { border-color: var(--border-accent); }
  .dev-info { display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0; }
  .dev-icon { font-size: 22px; flex-shrink: 0; }
  .dev-meta { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .dev-name { font-weight: 500; font-size: 13px; }
  .dev-detail { font-size: 11px; color: var(--text-dim); }
  .dev-mount { font-size: 11px; color: var(--accent); font-family: var(--font-mono); }
  .win-badge {
    font-size: 10px;
    background: rgba(0,120,212,0.2);
    color: #60aaff;
    border: 1px solid rgba(0,120,212,0.3);
    border-radius: 4px;
    padding: 1px 5px;
    width: fit-content;
  }
  .dev-actions { display: flex; gap: 6px; flex-shrink: 0; }
  .empty { text-align: center; color: var(--text-dim); padding: 24px; }
  .tip {
    margin: 0 10px 10px;
    padding: 10px 12px;
    background: var(--accent-dim);
    border: 1px solid var(--border-accent);
    border-radius: var(--radius);
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.6;
  }
  .tip code {
    background: var(--bg-tertiary);
    padding: 1px 5px; border-radius: 3px;
    font-family: var(--font-mono);
    color: var(--text-accent);
  }
  .panel-footer { padding: 10px; border-top: 1px solid var(--border); }
  .loading { padding: 24px; text-align: center; color: var(--text-dim); }

  /* ── Password Dialog ── */
  .pw-overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.55);
    
    
    display: flex; align-items: center; justify-content: center;
    z-index: 200;
  }
  .pw-dialog {
    width: 360px;
    border-radius: var(--radius-lg);
    padding: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 4px;
  }
  .pw-icon { font-size: 32px; margin-bottom: 6px; }
  .pw-title { font-size: 15px; font-weight: 600; color: var(--text-primary); }
  .pw-sub {
    font-size: 11px; color: var(--text-dim);
    margin: 6px 0 14px; line-height: 1.6;
  }
  .pw-sub code {
    font-family: var(--font-mono);
    color: var(--text-accent);
    background: var(--bg-tertiary);
    padding: 1px 4px; border-radius: 3px;
  }
  .pw-input {
    width: 100%;
    padding: 8px 12px;
    font-size: 13px;
  }
  .pw-error {
    color: var(--danger);
    font-size: 11px;
    margin-top: 8px;
  }
  .pw-actions {
    display: flex; gap: 8px;
    margin-top: 16px;
    width: 100%;
  }
  .pw-actions .btn { flex: 1; }
  .device-row.blink {
    animation: devblink 0.5s ease 3;
  }
  @keyframes devblink {
    0%,100% { background:var(--bg-tertiary); border-color:var(--border); }
    50%      { background:var(--accent-dim); border-color:var(--accent); box-shadow: 0 0 10px var(--accent); }
  }
</style>
