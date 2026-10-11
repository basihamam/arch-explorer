<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { navigateTo, notify } from "../stores/explorer.js";

  const dispatch = createEventDispatcher();

  let host        = "";
  let scanning    = false;
  let pinging     = false;
  let hostReach   = null; // null=bilinmiyor, true=erişilebilir, false=erişilemiyor
  let smb_shares  = [];
  let net_mounts  = [];
  let smbError    = "";
  let mountBusy   = null;

  // SMB auth
  let smbUser = "";
  let smbPass = "";
  let showAuth = false;
  let mountingShare = null;

  // NFS
  let nfsHost = "";
  let nfsPath = "";
  let nfsMounting = false;

  let activeTab = "smb"; // "smb" | "nfs" | "mounted"

  async function loadMounted() {
    try { net_mounts = await invoke("list_network_mounts"); }
    catch { net_mounts = []; }
  }

  $: if (activeTab === "mounted") loadMounted();

  async function pingHost() {
    if (!host.trim()) return;
    pinging = true; hostReach = null;
    try {
      hostReach = await invoke("ping_host", { host: host.trim() });
    } catch { hostReach = false; }
    finally { pinging = false; }
  }

  async function scanSmb() {
    if (!host.trim()) return;
    scanning = true; smb_shares = []; smbError = "";
    try {
      smb_shares = await invoke("list_smb_shares", { host: host.trim() });
      if (smb_shares.length === 0) smbError = "Paylaşım bulunamadı";
    } catch (e) {
      smbError = String(e);
    } finally { scanning = false; }
  }

  function openMountDialog(share) {
    mountingShare = share;
    showAuth = true;
    smbUser = ""; smbPass = "";
  }

  async function mountSmb() {
    if (!mountingShare) return;
    mountBusy = mountingShare.name;
    try {
      const result = await invoke("mount_smb", {
        host: mountingShare.host,
        share: mountingShare.name,
        username: smbUser || null,
        password: smbPass || null,
        mountPoint: null,
      });
      notify("success", result.message);
      if (result.mount_point) navigateTo(result.mount_point);
      showAuth = false;
      dispatch("close");
    } catch (e) {
      notify("error", String(e));
    } finally { mountBusy = null; }
  }

  async function mountNfs() {
    if (!nfsHost.trim() || !nfsPath.trim()) return;
    nfsMounting = true;
    try {
      const result = await invoke("mount_nfs", {
        host: nfsHost.trim(),
        remotePath: nfsPath.trim(),
        mountPoint: null,
      });
      notify("success", result.message);
      if (result.mount_point) navigateTo(result.mount_point);
      dispatch("close");
    } catch (e) {
      notify("error", String(e));
    } finally { nfsMounting = false; }
  }

  async function unmountNet(mp) {
    try {
      const msg = await invoke("unmount_network", { mountPoint: mp });
      notify("success", msg);
      await loadMounted();
    } catch (e) { notify("error", String(e)); }
  }
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">

    <div class="panel-header">
      <span>◈ Ağ Paylaşımları</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    <!-- Tab switcher -->
    <div class="tab-row">
      <button class="tab-btn" class:active={activeTab === "smb"} on:click={() => activeTab = "smb"}>SMB / Windows</button>
      <button class="tab-btn" class:active={activeTab === "nfs"} on:click={() => activeTab = "nfs"}>NFS / Linux</button>
      <button class="tab-btn" class:active={activeTab === "mounted"} on:click={() => activeTab = "mounted"}>Bağlı ({net_mounts.length})</button>
    </div>

    <div class="panel-body">

      {#if activeTab === "smb"}
        <!-- Host giriş -->
        <div class="host-row">
          <input
            class="host-input"
            bind:value={host}
            placeholder="Host adı veya IP (ör: 192.168.1.10 veya mypc)"
            on:keydown={e => e.key === "Enter" && scanSmb()}
          />
          <button class="btn" on:click={pingHost} disabled={pinging || !host.trim()}>
            {pinging ? "..." : "Ping"}
          </button>
          <button class="btn btn-accent" on:click={scanSmb} disabled={scanning || !host.trim()}>
            {scanning ? "Taranıyor..." : "Tara"}
          </button>
        </div>

        {#if hostReach !== null}
          <div class="reach-badge" class:ok={hostReach} class:fail={!hostReach}>
            {hostReach ? "◉ Erişilebilir" : "○ Erişilemiyor"}
          </div>
        {/if}

        {#if smbError}
          <div class="error-msg">⚠ {smbError}</div>
        {:else if smb_shares.length > 0}
          <div class="share-list">
            {#each smb_shares as share}
              <div class="share-row">
                <div class="share-info">
                  <span class="share-icon">{share.share_type === "Disk" ? "◉" : "🖨"}</span>
                  <div>
                    <div class="share-name">{share.name}</div>
                    {#if share.comment}<div class="share-comment">{share.comment}</div>{/if}
                  </div>
                </div>
                {#if share.share_type === "Disk"}
                  <button class="btn btn-accent" on:click={() => openMountDialog(share)} disabled={mountBusy === share.name}>
                    {mountBusy === share.name ? "..." : "Mount"}
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {:else if !scanning}
          <div class="state-msg">Host tara → paylaşımları listele</div>
        {/if}

        <!-- Auth dialog -->
        {#if showAuth && mountingShare}
          <div class="auth-box">
            <div class="auth-title">🔒 {mountingShare.host}/{mountingShare.name}</div>
            <label class="auth-label">
              Kullanıcı (boş = misafir)
              <input class="auth-input" bind:value={smbUser} placeholder="Kullanıcı adı" />
            </label>
            <label class="auth-label">
              Şifre
              <input class="auth-input" type="password" bind:value={smbPass} placeholder="Şifre" on:keydown={e => e.key === "Enter" && mountSmb()} />
            </label>
            <div class="auth-actions">
              <button class="btn" on:click={() => showAuth = false}>İptal</button>
              <button class="btn btn-accent" on:click={mountSmb} disabled={!!mountBusy}>
                {mountBusy ? "Mount ediliyor..." : "Mount Et"}
              </button>
            </div>
          </div>
        {/if}

      {:else if activeTab === "nfs"}
        <div class="nfs-form">
          <label class="nfs-label">
            NFS Sunucusu
            <input class="host-input" bind:value={nfsHost} placeholder="192.168.1.10 veya nfsserver" />
          </label>
          <label class="nfs-label">
            Uzak Yol
            <input class="host-input" bind:value={nfsPath} placeholder="/exports/data" />
          </label>
          <div class="nfs-hint">Mount: <code>{nfsHost || "host"}:{nfsPath || "/path"}</code> → <code>/run/media/network/...</code></div>
          <button class="btn btn-accent" on:click={mountNfs} disabled={nfsMounting || !nfsHost.trim() || !nfsPath.trim()}>
            {nfsMounting ? "Mount ediliyor..." : "NFS Mount Et"}
          </button>
        </div>

        <div class="pkg-hint">
          <strong>Gereksinimler:</strong><br>
          NFS: <code>sudo pacman -S nfs-utils</code><br>
          SMB: <code>sudo pacman -S cifs-utils smbclient</code>
        </div>

      {:else}
        <!-- Bağlı paylaşımlar -->
        {#if net_mounts.length === 0}
          <div class="state-msg">Bağlı ağ paylaşımı yok</div>
        {:else}
          <div class="share-list">
            {#each net_mounts as m}
              <div class="share-row">
                <div class="share-info">
                  <span class="share-icon">{m.fs_type === "cifs" ? "🪟" : "◉"}</span>
                  <div>
                    <div class="share-name">{m.source}</div>
                    <div class="share-comment">{m.mount_point} · {m.fs_type}</div>
                  </div>
                </div>
                <div style="display:flex;gap:5px">
                  <button class="btn" on:click={() => { navigateTo(m.mount_point); dispatch("close"); }}>Aç</button>
                  <button class="btn btn-danger" on:click={() => unmountNet(m.mount_point)}>Ayır</button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}

    </div>
  </div>
</div>

<style>
  .overlay { position:fixed; inset:0; background:rgba(0,0,0,0.45);   display:flex; align-items:center; justify-content:center; z-index:110; }
  .panel { width:520px; max-height:78vh; border-radius:var(--radius-lg); display:flex; flex-direction:column; overflow:hidden; }
  .panel-header { display:flex; justify-content:space-between; align-items:center; padding:14px 18px; border-bottom:1px solid var(--border); font-weight:600; font-size:13px; flex-shrink:0; }
  .panel-header button { background:none; border:none; color:var(--text-dim); cursor:pointer; font-size:14px; }

  .tab-row { display:flex; border-bottom:1px solid var(--border); flex-shrink:0; }
  .tab-btn { flex:1; padding:8px; background:none; border:none; border-bottom:2px solid transparent; color:var(--text-dim); font-size:12px; cursor:pointer; transition:color 0.1s, border-color 0.1s; }
  .tab-btn:hover { color:var(--text-secondary); }
  .tab-btn.active { color:var(--accent); border-bottom-color:var(--accent); }

  .panel-body { flex:1; overflow-y:auto; padding:14px; display:flex; flex-direction:column; gap:10px; }

  .host-row { display:flex; gap:6px; align-items:center; }
  .host-input { flex:1; padding:7px 10px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); color:var(--text-primary); font-size:12px; }
  .host-input:focus { border-color:var(--border-accent); outline:none; }

  .reach-badge { font-size:11px; font-weight:600; padding:4px 8px; border-radius:4px; width:fit-content; }
  .reach-badge.ok { color:var(--success); background:rgba(90,255,142,0.1); border:1px solid rgba(90,255,142,0.3); }
  .reach-badge.fail { color:var(--danger); background:rgba(255,95,109,0.1); border:1px solid rgba(255,95,109,0.3); }

  .error-msg { color:var(--danger); font-size:11px; padding:8px; background:rgba(255,95,109,0.08); border:1px solid rgba(255,95,109,0.2); border-radius:var(--radius-sm); }
  .state-msg { text-align:center; color:var(--text-dim); font-size:12px; padding:20px; }

  .share-list { display:flex; flex-direction:column; gap:5px; }
  .share-row { display:flex; align-items:center; justify-content:space-between; padding:10px 12px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); gap:10px; }
  .share-info { display:flex; align-items:center; gap:10px; flex:1; min-width:0; }
  .share-icon { font-size:18px; flex-shrink:0; }
  .share-name { font-size:13px; font-weight:500; color:var(--text-primary); }
  .share-comment { font-size:10px; color:var(--text-dim); }

  .auth-box { background:var(--bg-tertiary); border:1px solid var(--border-accent); border-radius:var(--radius); padding:14px; display:flex; flex-direction:column; gap:8px; }
  .auth-title { font-size:12px; font-weight:600; color:var(--text-primary); }
  .auth-label { display:flex; flex-direction:column; gap:4px; font-size:11px; color:var(--text-dim); }
  .auth-input { padding:6px 8px; background:var(--bg-secondary); border:1px solid var(--border); border-radius:var(--radius-sm); color:var(--text-primary); font-size:12px; }
  .auth-input:focus { border-color:var(--border-accent); outline:none; }
  .auth-actions { display:flex; gap:8px; justify-content:flex-end; }

  .nfs-form { display:flex; flex-direction:column; gap:10px; }
  .nfs-label { display:flex; flex-direction:column; gap:4px; font-size:11px; color:var(--text-dim); }
  .nfs-hint { font-size:11px; color:var(--text-dim); }
  .nfs-hint code { font-family:var(--font-mono); color:var(--text-accent); background:var(--bg-tertiary); padding:1px 4px; border-radius:3px; }

  .pkg-hint { font-size:11px; color:var(--text-dim); padding:10px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); line-height:1.8; }
  .pkg-hint code { font-family:var(--font-mono); color:var(--text-accent); background:var(--bg-secondary); padding:1px 4px; border-radius:3px; }
  .pkg-hint strong { color:var(--text-secondary); }
</style>
