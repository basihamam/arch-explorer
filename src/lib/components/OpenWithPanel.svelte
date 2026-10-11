<script>
  import { invoke } from "@tauri-apps/api/core";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { createEventDispatcher, onMount } from "svelte";
  import { notify } from "../stores/explorer.js";

  export let filePath = "";
  export let fileName = "";

  const dispatch = createEventDispatcher();

  let info       = null;
  let loading    = true;
  let selected   = null;
  let searchQ    = "";
  let customCmd  = "";
  let error      = null;

  onMount(async () => {
    if (!filePath) { error = "No file path"; loading = false; return; }
    try {
      info = await invoke("get_open_with_info", { path: filePath });
      selected = info.default_app?.id ?? info.recommended[0]?.id ?? null;
    } catch (e) { error = String(e); }
    finally { loading = false; }
  });

  $: allSections = !info ? [] : [
    ...(info.default_app ? [{ __section: "Default" }, info.default_app] : []),
    ...(info.recommended.length ? [{ __section: "Recommended Applications" }, ...info.recommended] : []),
    ...(info.others.length ? [{ __section: "Other Applications" }, ...info.others] : []),
    ...(info.custom.length ? [{ __section: "Custom Commands & Scripts" }, ...info.custom] : []),
  ];

  $: filtered = allSections.filter(item =>
    item.__section ||
    !searchQ ||
    item.name?.toLowerCase().includes(searchQ.toLowerCase())
  );

  function iconSrc(app) {
    if (!app?.icon_path) return null;
    try { return convertFileSrc(app.icon_path); } catch { return null; }
  }

  function avatarColor(name) {
    const colors = ["#7c6af7","#f76a8c","#6af7c2","#f7b46a","#6ac8f7","#c46af7","#f7e06a","#6af78c"];
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h*31+name.charCodeAt(i))&0xffff;
    return colors[h%colors.length];
  }

  function avatarLetter(name) { return name?.[0]?.toUpperCase() ?? "?"; }

  async function browseExecutable() {
    try {
      const sel = await open({ multiple: false, directory: false });
      if (sel) customCmd = typeof sel === "string" ? sel : sel.path;
    } catch(e) { notify("error", String(e)); }
  }

  async function openOnce() {
    if (customCmd.trim()) {
      try {
        await invoke("open_with_custom_cmd", { path: filePath, customCmd: customCmd.trim() });
        dispatch("close");
      } catch(e) { notify("error", String(e)); }
      return;
    }
    if (!selected) return;
    try {
      // Eğer dosya türü için önceden atanmış bir varsayılan uygulama yoksa,
      // seçilen bu uygulamayı varsayılan olarak kaydet (kullanıcı kuralı).
      // Ancak zaten bir varsayılan atanmışsa üzerine yazma!
      if (!info?.default_app && info?.mime_type && !selected.startsWith("custom::")) {
        await invoke("set_default_app", { mimeType: info.mime_type, desktopId: selected }).catch((e) => {
          console.warn("Auto set_default_app failed:", e);
        });
      }
      await invoke("open_with_app", { path: filePath, desktopId: selected });
      dispatch("close");
    } catch(e) { notify("error", String(e)); }
  }

  async function setDefault() {
    if (!selected || !info) return;
    try {
      await invoke("set_default_app", { mimeType: info.mime_type, desktopId: selected });
      info = await invoke("get_open_with_info", { path: filePath });
      selected = info.default_app?.id;
      notify("success", "Varsayılan uygulama ayarlandı");
    } catch(e) { notify("error", String(e)); }
  }

  async function resetDefaults() {
    if (!info) return;
    try {
      await invoke("reset_default_for_mime", { mimeType: info.mime_type });
      info = await invoke("get_open_with_info", { path: filePath });
      notify("success", "Sistem varsayılanına sıfırlandı");
    } catch(e) { notify("error", String(e)); }
  }

  function handleKey(e) {
    if (e.key === "Escape") dispatch("close");
    if (e.key === "Enter" && (selected || customCmd.trim())) openOnce();
  }
</script>

<svelte:window on:keydown={handleKey} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="ow-backdrop" on:click={() => dispatch("close")}>
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="ow-panel glass" on:click|stopPropagation>

    <div class="ow-header">
      <div class="ow-header-left">
        <div class="ow-file-icon">
          <span class="mime-badge">{info?.mime_type.split('/')[1]?.slice(0,4) ?? '···'}</span>
        </div>
        <div class="ow-header-text">
          <div class="ow-title">Open With</div>
          <div class="ow-filename" title={filePath}>{fileName || filePath.split('/').pop()}</div>
          {#if info}<div class="ow-mime">{info.mime_type}</div>{/if}
        </div>
      </div>
      <button class="ow-close" on:click={() => dispatch("close")}>✕</button>
    </div>

    <!-- Search -->
    <div class="ow-search-wrap">
      <span class="ow-search-icon">◎</span>
      <input class="ow-search" placeholder="Search apps…" bind:value={searchQ} autofocus />
    </div>

    <!-- Single unified list -->
    <div class="ow-list">
      {#if loading}
        <div class="ow-state"><div class="ow-spinner"></div></div>
      {:else if error}
        <div class="ow-state">⚠ {error}</div>
      {:else}
        {#each filtered as item (item.__section ? '__sec__'+item.__section : item.id)}
          {#if item.__section}
            <div class="ow-section-label">{item.__section}</div>
          {:else}
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div
              class="ow-app"
              class:selected={selected === item.id}
              class:is-default={info?.default_app?.id === item.id}
              on:click={() => { selected = item.id; customCmd = ""; }}
              on:dblclick={openOnce}
            >
              {#if iconSrc(item)}
                <img class="ow-icon" src={iconSrc(item)} alt=""
                  on:error={e => { e.target.style.display="none"; e.target.nextElementSibling.style.display="flex"; }} />
                <div class="ow-avatar" style="background:{avatarColor(item.name)};display:none">
                  {avatarLetter(item.name)}
                </div>
              {:else}
                <div class="ow-avatar" style="background:{avatarColor(item.name)}">
                  {item.is_custom ? (item.icon ?? avatarLetter(item.name)) : avatarLetter(item.name)}
                </div>
              {/if}
              <div class="ow-app-info">
                <span class="ow-app-name">{item.name}</span>
                {#if item.comment || item.is_custom}
                  <span class="ow-app-comment">{item.is_custom ? item.exec : item.comment}</span>
                {/if}
              </div>
              {#if info?.default_app?.id === item.id}
                <span class="ow-default-badge">default</span>
              {/if}
              {#if selected === item.id}<span class="ow-check">✓</span>{/if}
            </div>
          {/if}
        {/each}
        {#if !loading && filtered.filter(i => !i.__section).length === 0}
          <div class="ow-state">No application found</div>
        {/if}
      {/if}
    </div>

    <!-- Custom cmd -->
    <div class="ow-custom-box">
      <span class="ow-custom-hint">Or run an executable command directly:</span>
      <div class="ow-custom-row">
        <input type="text" class="ow-custom-input" placeholder="Executable or command…" bind:value={customCmd} />
        <button class="ow-browse-btn" on:click={browseExecutable}>📁</button>
      </div>
    </div>

    <!-- Footer -->
    <div class="ow-footer">
      <button class="btn-ghost danger" on:click={resetDefaults}>↺ Reset to System Default</button>
      <div class="ow-footer-right">
        <button class="btn-ghost" on:click={() => dispatch("close")}>Cancel</button>
        <button class="btn-secondary" on:click={setDefault} disabled={!selected}>★ Set as Default</button>
        <button class="btn-primary" on:click={openOnce} disabled={!selected && !customCmd.trim()}>▶ Open</button>
      </div>
    </div>
  </div>
</div>

<style>
  .ow-backdrop { position:fixed;inset:0;z-index:10000;background:rgba(0,0,0,0.45);display:flex;align-items:center;justify-content:center;animation:owf .15s ease; }
  @keyframes owf{from{opacity:0}to{opacity:1}}
  .ow-panel { width:560px;max-width:calc(100vw - 32px);max-height:calc(100vh - 64px);display:flex;flex-direction:column;border-radius:14px;overflow:hidden;animation:owu .18s cubic-bezier(.16,1,.3,1);box-shadow:0 24px 64px rgba(0,0,0,0.6),0 0 0 1px rgba(255,255,255,0.07); }
  @keyframes owu{from{transform:translateY(18px) scale(0.97);opacity:0}to{transform:none;opacity:1}}
  .ow-header { display:flex;align-items:center;justify-content:space-between;padding:14px 18px 10px;border-bottom:1px solid var(--border);flex-shrink:0; }
  .ow-header-left { display:flex;align-items:center;gap:12px;min-width:0; }
  .ow-file-icon { width:42px;height:42px;border-radius:10px;background:linear-gradient(135deg,var(--accent),color-mix(in srgb,var(--accent) 60%,#000));display:flex;align-items:center;justify-content:center;flex-shrink:0; }
  .mime-badge { font-size:10px;font-weight:700;color:#fff;text-transform:uppercase;letter-spacing:.05em; }
  .ow-title { font-size:13px;font-weight:700;color:var(--text-primary); }
  .ow-filename { font-size:11.5px;color:var(--text-secondary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:340px; }
  .ow-mime { font-size:10px;color:var(--text-dim);font-family:var(--font-mono); }
  .ow-close { width:28px;height:28px;border-radius:50%;border:none;background:var(--bg-hover);color:var(--text-dim);cursor:pointer;font-size:12px;display:flex;align-items:center;justify-content:center;flex-shrink:0;transition:var(--transition-fast); }
  .ow-close:hover { background:var(--danger-soft);color:var(--danger); }
  .ow-search-wrap { display:flex;align-items:center;gap:8px;padding:9px 16px;border-bottom:1px solid var(--border);flex-shrink:0; }
  .ow-search-icon { font-size:13px;color:var(--text-dim); }
  .ow-search { flex:1;background:none;border:none;outline:none;font-size:13px;color:var(--text-primary);font-family:inherit; }
  .ow-search::placeholder { color:var(--text-dim); }
  .ow-list { flex:1;overflow-y:auto;padding:6px 8px;min-height:220px;max-height:380px; }
  .ow-list::-webkit-scrollbar { width:4px; }
  .ow-list::-webkit-scrollbar-thumb { background:var(--border);border-radius:2px; }
  .ow-section-label { padding:10px 12px 4px;font-size:10px;font-weight:700;color:var(--text-dim);text-transform:uppercase;letter-spacing:.1em; }
  .ow-state { display:flex;align-items:center;justify-content:center;gap:8px;padding:30px;color:var(--text-dim);font-size:12px; }
  .ow-spinner { width:18px;height:18px;border:2px solid var(--border);border-top-color:var(--accent);border-radius:50%;animation:spin .7s linear infinite; }
  @keyframes spin{to{transform:rotate(360deg)}}
  .ow-app { display:flex;align-items:center;gap:14px;padding:9px 12px;border-radius:var(--radius);cursor:pointer;transition:var(--transition-fast);position:relative;min-height:56px; }
  .ow-app:hover { background:var(--bg-hover); }
  .ow-app.selected { background:color-mix(in srgb,var(--accent) 16%,transparent); }
  .ow-app.is-default::before { content:'';position:absolute;left:0;top:15%;bottom:15%;width:3px;border-radius:2px;background:var(--accent); }
  .ow-icon { width:48px;height:48px;object-fit:contain;border-radius:9px;flex-shrink:0; }
  .ow-avatar { width:48px;height:48px;border-radius:9px;display:flex;align-items:center;justify-content:center;font-weight:700;font-size:20px;color:#fff;flex-shrink:0; }
  .ow-app-info { flex:1;min-width:0;display:flex;flex-direction:column;gap:3px; }
  .ow-app-name { font-size:13.5px;font-weight:600;color:var(--text-primary);white-space:nowrap;overflow:hidden;text-overflow:ellipsis; }
  .ow-app-comment { font-size:11.5px;color:var(--text-dim);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-family:var(--font-mono); }
  .ow-default-badge { font-size:9px;padding:2px 6px;background:var(--accent);color:#fff;border-radius:4px;flex-shrink:0;font-weight:600; }
  .ow-check { color:var(--accent);font-size:15px;font-weight:700;flex-shrink:0; }
  .ow-custom-box { padding:10px 16px;border-top:1px solid var(--border);display:flex;flex-direction:column;gap:6px;flex-shrink:0; }
  .ow-custom-hint { font-size:11px;color:var(--text-dim); }
  .ow-custom-row { display:flex;gap:6px; }
  .ow-custom-input { flex:1;background:var(--bg-hover);border:1px solid var(--border);border-radius:var(--radius-sm);padding:6px 10px;font-size:12px;color:var(--text-primary);outline:none;transition:var(--transition-fast); }
  .ow-custom-input:focus { border-color:var(--accent); }
  .ow-browse-btn { width:32px;height:32px;border-radius:var(--radius-sm);border:1px solid var(--border);background:var(--bg-hover);color:var(--text-primary);cursor:pointer;display:flex;align-items:center;justify-content:center;font-size:14px;transition:var(--transition-fast);flex-shrink:0; }
  .ow-browse-btn:hover { border-color:var(--accent); }
  .ow-footer { display:flex;align-items:center;justify-content:space-between;padding:10px 16px;border-top:1px solid var(--border);flex-shrink:0;gap:8px; }
  .ow-footer-right { display:flex;align-items:center;gap:8px; }
  .btn-ghost,.btn-secondary,.btn-primary { border:none;border-radius:var(--radius);padding:6px 13px;font-size:12px;font-weight:500;cursor:pointer;transition:var(--transition-fast);font-family:inherit;white-space:nowrap; }
  .btn-ghost { background:none;color:var(--text-secondary); }
  .btn-ghost:hover { background:var(--bg-hover);color:var(--text-primary); }
  .btn-ghost.danger { color:var(--danger); }
  .btn-ghost.danger:hover { background:var(--danger-soft); }
  .btn-secondary { background:var(--bg-hover);color:var(--text-primary);border:1px solid var(--border); }
  .btn-secondary:hover { background:var(--bg-tertiary); }
  .btn-primary { background:var(--accent);color:#fff; }
  .btn-primary:hover { filter:brightness(1.12); }
  .btn-primary:disabled,.btn-secondary:disabled { opacity:.35;cursor:not-allowed; }
</style>
