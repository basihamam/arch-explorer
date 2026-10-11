<script>
  import { contextMenu, selectedPaths, currentPath,
           loadDirectory, deleteItems, runCommand,
           userCommands, notify, navigateTo,
           copySelected, cutSelected, pasteClipboard,
           clipboard, addBookmark,
           previewEntry, showPreview, addTab,
           entries, renamingPath, requestSudo } from "../stores/explorer.js";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { onMount, createEventDispatcher } from "svelte";
  import OpenWithPanel from "./OpenWithPanel.svelte";

  const dispatch = createEventDispatcher();

  let menu;
  let showOpenWith = false;
  let openWithFile = null;   // { path, name }

  // ── Open With submenu ──
  let owApps       = null;  // { default_app, recommended, others, custom }
  let owLoading    = false;
  let owTimer      = null;
  let owVisible    = false;

  async function showOwSubmenu() {
    if (!target) return;
    clearTimeout(owTimer);
    owVisible = true;
    if (!owApps && !owLoading) {
      owLoading = true;
      try {
        owApps = await invoke("get_open_with_info", { path: target.path });
      } catch {
        owApps = null;
      } finally {
        owLoading = false;
      }
    }
  }

  function hideOwSubmenu() {
    clearTimeout(owTimer);
    owTimer = setTimeout(() => {
      owVisible = false;
    }, 250);
  }

  function keepOw() {
    clearTimeout(owTimer);
    owVisible = true;
  }

  async function quickOpenWith(app) {
    contextMenu.set(null);
    owVisible = false;
    try {
      await invoke("open_with_app", { path: target.path, desktopId: app.id });
    } catch(e) {
      notify("error", String(e));
    }
  }

  function owIconSrc(app) {
    if (!app?.icon_path) return null;
    try {
      return convertFileSrc(app.icon_path);
    } catch {
      return null;
    }
  }

  // ── Command Icon Resolver ──
  const CMD_ICONS = {
    archive: "📦",
    shield: "🛡",
    terminal: "⬛",
    copy: "⎘",
    git: "⌥",
    edit: "✏",
    trash: "🗑",
    code: "💻",
    link: "🔗",
    search: "🔍",
  };
  function resolveCmdIcon(icon) {
    if (!icon) return "⚡";
    return CMD_ICONS[icon.toLowerCase()] || icon;
  }

  // ── Archive compress dialog ──
  let showCompress   = false;
  let compressName   = "";
  let compressFormat = "zip";
  let compressBusy   = false;

  // ── Checksum dialog ──
  let showChecksum = false;
  let checksumData = null;
  let checksumBusy = false;

  onMount(() => {
    const close = (e) => {
      if (menu && e.target && menu.contains(e.target)) return;
      contextMenu.set(null);
    };
    window.addEventListener("pointerdown", close, { capture: true });
    window.addEventListener("click", close, { capture: true });
    window.addEventListener("contextmenu", close, { capture: true });
    window.addEventListener("keydown", e => { if (e.key === "Escape") contextMenu.set(null); });
    return () => {
      window.removeEventListener("pointerdown", close, { capture: true });
      window.removeEventListener("click", close, { capture: true });
      window.removeEventListener("contextmenu", close, { capture: true });
    };
  });

  $: target          = $contextMenu?.target;
  $: selectedList    = [...$selectedPaths];
  $: hasClipboard    = $clipboard.paths.length > 0;
  $: contextCmds     = $userCommands.filter(c => c.show_in_context_menu);
  $: multiSelect     = selectedList.length > 1;
  $: targetIsArchive = target && !target.is_dir &&
       /\.(zip|tar\.gz|tgz|tar\.bz2|tbz2|tar\.xz|txz|tar\.zst|tar|7z|rar)$/i.test(target.name);

  function openInNewTab() {
    if (!target) return;
    addTab(target.path);
    contextMenu.set(null);
  }

  // ── Actions ──
  function openWith() {
    if (!target) return;
    openWithFile = { path: target.path, name: target.name };
    showOpenWith = true;
    contextMenu.set(null);
  }

  function getUniqueItemName(baseName, isDir) {
    const existingNames = new Set(($entries || []).map(e => e.name));
    if (!existingNames.has(baseName)) return baseName;
    let count = 2;
    let dotIdx = isDir ? -1 : baseName.lastIndexOf(".");
    while (true) {
      let candidate;
      if (dotIdx > 0) {
        const pre = baseName.slice(0, dotIdx);
        const ext = baseName.slice(dotIdx);
        candidate = `${pre} (${count})${ext}`;
      } else {
        candidate = `${baseName} (${count})`;
      }
      if (!existingNames.has(candidate)) return candidate;
      count++;
    }
  }

  async function newFolder() {
    const base = "New Folder";
    const name = getUniqueItemName(base, true);
    const targetDir = target?.is_dir ? target.path : $currentPath;
    const newPath = targetDir.replace(/\/+$/, "") + "/" + name;
    contextMenu.set(null);
    try {
      await invoke("create_dir", { path: newPath });
      await loadDirectory($currentPath);
      selectedPaths.set(new Set([newPath]));
      renamingPath.set(newPath);
    } catch (e) {
      const errStr = String(e).toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("access denied")) {
        try {
          await requestSudo(
            "Klasör Oluşturma İçin Yetki Gerekli",
            `'${newPath}' oluşturmak için root yetkisi gereklidir.`,
            async (password) => {
              await invoke("sudo_create_dir", { path: newPath, password });
            }
          );
          await loadDirectory($currentPath);
          selectedPaths.set(new Set([newPath]));
          renamingPath.set(newPath);
        } catch (sudoErr) {
          notify("error", String(sudoErr));
        }
      } else {
        notify("error", String(e));
      }
    }
  }

  async function newFile() {
    const base = "New File";
    const name = getUniqueItemName(base, false);
    const targetDir = target?.is_dir ? target.path : $currentPath;
    const newPath = targetDir.replace(/\/+$/, "") + "/" + name;
    contextMenu.set(null);
    try {
      await invoke("create_file", { path: newPath });
      await loadDirectory($currentPath);
      selectedPaths.set(new Set([newPath]));
      renamingPath.set(newPath);
    } catch (e) {
      const errStr = String(e).toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("access denied")) {
        try {
          await requestSudo(
            "Dosya Oluşturma İçin Yetki Gerekli",
            `'${newPath}' oluşturmak için root yetkisi gereklidir.`,
            async (password) => {
              await invoke("sudo_create_file", { path: newPath, password });
            }
          );
          await loadDirectory($currentPath);
          selectedPaths.set(new Set([newPath]));
          renamingPath.set(newPath);
        } catch (sudoErr) {
          notify("error", String(sudoErr));
        }
      } else {
        notify("error", String(e));
      }
    }
  }

  async function rename() {
    if (!target) return;
    contextMenu.set(null);
    selectedPaths.set(new Set([target.path]));
    renamingPath.set(target.path);
  }

  function copyItems() {
    if (target && !$selectedPaths.has(target.path)) {
      selectedPaths.update(s => { const n = new Set(s); n.add(target.path); return n; });
    }
    copySelected();
    contextMenu.set(null);
  }

  function cutItems() {
    if (target && !$selectedPaths.has(target.path)) {
      selectedPaths.update(s => { const n = new Set(s); n.add(target.path); return n; });
    }
    cutSelected();
    contextMenu.set(null);
  }

  async function paste() {
    await pasteClipboard();
    contextMenu.set(null);
  }

  async function trash() {
    const paths = selectedList.length ? selectedList : [target?.path].filter(Boolean);
    await deleteItems(paths, false);
    contextMenu.set(null);
  }

  async function deletePerma() {
    const paths = selectedList.length ? selectedList : [target?.path].filter(Boolean);
    if (!confirm(`Permanently delete ${paths.length} item(s)?`)) return;
    await deleteItems(paths, true);
    contextMenu.set(null);
  }

  function copyPath() {
    const p = target?.path || $currentPath;
    navigator.clipboard.writeText(p);
    notify("success", "Path copied");
    contextMenu.set(null);
  }

  async function bookmarkCurrent() {
    const name = $currentPath.split("/").filter(Boolean).pop() || "root";
    await addBookmark(name, $currentPath);
    contextMenu.set(null);
  }

  async function openTerminal() {
    const dir = target?.is_dir ? target.path : (target ? target.path.replace(/\/[^/]+$/, "") : $currentPath);
    try {
      await invoke("run_user_command", {
        commandId: "open-terminal",
        selectedPath: dir,
        currentDir: dir,
      });
    } catch {
      // Fallback: directly spawn kitty
      await invoke("plugin:shell|execute", {
        program: "kitty",
        args: ["--directory", dir],
        options: {}
      }).catch(() => notify("error", "Cannot open terminal"));
    }
    contextMenu.set(null);
  }

  async function runCmd(cmd) {
    await runCommand(cmd.id, target?.path);
    contextMenu.set(null);
  }

  // ── Archive: Extract ──
  async function extract() {
    const path = target?.path;
    if (!path) return;
    contextMenu.set(null);
    try {
      notify("info", "Çıkarılıyor...");
      const result = await invoke("extract_archive", { archivePath: path });
      notify("success", result.message);
      await loadDirectory($currentPath);
    } catch (e) {
      notify("error", String(e));
    }
  }

  // ── Archive: Compress dialog ──
  function openCompress() {
    const paths = selectedList.length ? selectedList : [target?.path].filter(Boolean);
    if (!paths.length) return;
    const first  = paths[0].split("/").pop() || "archive";
    const dotIdx = first.lastIndexOf(".");
    compressName   = dotIdx > 0 ? first.slice(0, dotIdx) : first;
    compressFormat = "zip";
    showCompress   = true;
  }

  async function doCompress() {
    const paths = selectedList.length ? selectedList : [target?.path].filter(Boolean);
    if (!paths.length || !compressName) return;
    compressBusy = true;
    try {
      const result = await invoke("compress_files", {
        paths,
        destDir: $currentPath,
        archiveName: compressName,
        format: compressFormat,
      });
      notify("success", result.message);
      await loadDirectory($currentPath);
      showCompress = false;
      contextMenu.set(null);
    } catch (e) {
      notify("error", String(e));
    } finally {
      compressBusy = false;
    }
  }

  // ── Checksum ──
  async function showChecksumFor() {
    const path = target?.path;
    if (!path) return;
    contextMenu.set(null);
    checksumBusy = true;
    showChecksum = true;
    checksumData = null;
    try {
      checksumData = await invoke("get_checksum", { path });
    } catch (e) {
      notify("error", String(e));
      showChecksum = false;
    } finally {
      checksumBusy = false;
    }
  }

  // ── Bulk rename ──
  function openBulkRename() {
    contextMenu.set(null);
    dispatch("bulk-rename");
  }

  $: if ($contextMenu) {
    // Menü her yeni açıldığında alt menüler kapalı başlasın!
    moreVisible = false;
    owVisible = false;
    owApps = null;
    clearTimeout(moreTimer);
    clearTimeout(owTimer);
    // Arka planda anında prefetch yap, kullanıcı üzerine geldiğinde beklemesin!
    if ($contextMenu.target && !$contextMenu.target.is_dir) {
      invoke("get_open_with_info", { path: $contextMenu.target.path })
        .then(res => { owApps = res; })
        .catch(() => {});
    }
  }

  // ── More Options submenu state ──
  let moreVisible = false;
  let moreTimer   = null;

  function showMoreSubmenu() {
    clearTimeout(moreTimer);
    moreTimer = setTimeout(() => {
      moreVisible = true;
    }, 80);
  }

  function hideMoreSubmenu() {
    clearTimeout(moreTimer);
    moreTimer = setTimeout(() => {
      moreVisible = false;
    }, 220);
  }

  function keepMore() {
    clearTimeout(moreTimer);
  }

  let isNearRightEdge = false;
  let isNearBottomEdge = false;
  // Position: keep menu inside viewport with safe clamping and scrolling
  $: menuStyle = (() => {
    if (!$contextMenu) return "";
    const x  = $contextMenu.x;
    const y  = $contextMenu.y;
    const vw = window.innerWidth || 1000;
    const vh = window.innerHeight || 700;
    const mw = 220;
    const mh = 360;
    isNearRightEdge = (x + mw + 260 > vw);
    isNearBottomEdge = (y + mh > vh);
    const left = (x + mw > vw) ? Math.max(8, x - mw) : Math.max(8, x);
    const top  = (y + mh > vh) ? Math.max(8, y - mh) : Math.max(8, y);
    return `left:${left}px; top:${top}px`;
  })();
</script>

{#if $contextMenu}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="context-menu glass"
    style={menuStyle}
    bind:this={menu}
    on:click|stopPropagation
    on:contextmenu|preventDefault|stopPropagation
  >
    {#if target}
      <div class="ctx-label truncate" title={target.path}>{target.name}</div>
      <div class="ctx-sep"></div>

      <!-- ── PRIMARY COMMANDS (Windows 11 Style) ── -->
      {#if target.is_dir}
        <button class="ctx-item" on:click={openInNewTab}>
          <span class="ctx-icon">⊞</span>
          <span class="ctx-label">Open in New Tab</span>
          <span class="ctx-shortcut">Mid-click</span>
        </button>
      {/if}

      <!-- ── OPEN WITH (Ana menüde!) ── -->
      {#if !target.is_dir}
        <div
          class="ctx-submenu-wrap"
          on:mouseenter={showOwSubmenu}
          on:mouseleave={hideOwSubmenu}
        >
          <button class="ctx-item" on:click={openWith}>
            <span class="ctx-icon">▶</span>
            <span class="ctx-label">Birlikte Aç (Open With)</span>
            <span class="ctx-arrow">›</span>
          </button>

          {#if owVisible}
            <div
              class="ctx-submenu glass"
              class:flip-left={isNearRightEdge}
              class:flip-top={isNearBottomEdge}
              on:mouseenter={keepOw}
              on:mouseleave={hideOwSubmenu}
            >
              {#if owLoading}
                <div class="ctx-ow-loading"><div class="ctx-ow-spinner"></div></div>
              {:else if owApps}
                {#if owApps.default_app}
                  <button class="ctx-item ctx-ow-item" on:click={() => quickOpenWith(owApps.default_app)}>
                    {#if owIconSrc(owApps.default_app)}
                      <img class="ctx-ow-icon" src={owIconSrc(owApps.default_app)} alt="" on:error={e=>e.target.style.display="none"} />
                    {:else}<span class="ctx-icon">▶</span>{/if}
                    <span class="ctx-ow-name">{owApps.default_app.name}</span>
                    <span class="ctx-ow-badge">default</span>
                  </button>
                {/if}
                {#if owApps.recommended && owApps.recommended.length > 0}
                  <div class="ctx-sep"></div>
                  <div class="ctx-ow-label">Recommended</div>
                  {#each owApps.recommended.slice(0,6) as app}
                    <button class="ctx-item ctx-ow-item" on:click={() => quickOpenWith(app)}>
                      {#if owIconSrc(app)}
                        <img class="ctx-ow-icon" src={owIconSrc(app)} alt="" on:error={e=>e.target.style.display="none"} />
                      {:else}<span class="ctx-icon">▶</span>{/if}
                      <span class="ctx-ow-name">{app.name}</span>
                    </button>
                  {/each}
                {/if}
                <div class="ctx-sep"></div>
                <button class="ctx-item" on:click={openWith}>
                  <span class="ctx-icon">⊞</span>
                  <span class="ctx-ow-name">Tüm Uygulamalar…</span>
                </button>
              {:else}
                <button class="ctx-item" on:click={openWith}>
                  <span class="ctx-icon">⊞</span>
                  <span class="ctx-ow-name">Uygulama Seç…</span>
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/if}

      <button class="ctx-item" on:click={copyItems}>
        <span class="ctx-icon">⎘</span>
        <span class="ctx-label">Copy</span>
        <span class="ctx-shortcut">Ctrl+C</span>
      </button>
      <button class="ctx-item" on:click={cutItems}>
        <span class="ctx-icon">✂</span>
        <span class="ctx-label">Cut</span>
        <span class="ctx-shortcut">Ctrl+X</span>
      </button>
      {#if hasClipboard}
        <button class="ctx-item" on:click={paste}>
          <span class="ctx-icon">📋</span>
          <span class="ctx-label">Paste</span>
          <span class="ctx-shortcut">Ctrl+V</span>
        </button>
      {/if}
      <button class="ctx-item" on:click={rename}>
        <span class="ctx-icon">✏</span>
        <span class="ctx-label">Rename</span>
        <span class="ctx-shortcut">F2</span>
      </button>
      <button class="ctx-item" on:click={newFolder}>
        <span class="ctx-icon">📁</span>
        <span class="ctx-label">New Folder</span>
      </button>
      <button class="ctx-item" on:click={newFile}>
        <span class="ctx-icon">📄</span>
        <span class="ctx-label">New File Here</span>
      </button>

      <div class="ctx-sep"></div>

      <button class="ctx-item" on:click={trash}>
        <span class="ctx-icon">🗑</span>
        <span class="ctx-label">Move to Trash</span>
        <span class="ctx-shortcut">Del</span>
      </button>
      <button class="ctx-item danger" on:click={deletePerma}>
        <span class="ctx-icon">✕</span>
        <span class="ctx-label">Delete Permanently</span>
        <span class="ctx-shortcut">Shift+Del</span>
      </button>

      <div class="ctx-sep"></div>

      <!-- ── MORE OPTIONS / DİĞER SEÇENEKLER (Secondary Tools & Commands) ── -->
      <div
        class="ctx-submenu-wrap"
        on:mouseenter={showMoreSubmenu}
        on:mouseleave={hideMoreSubmenu}
      >
        <button class="ctx-item more-item" on:click={() => moreVisible = !moreVisible}>
          <span class="ctx-icon">⋯</span>
          <span class="ctx-label">Diğer Seçenekler</span>
          <span class="ctx-arrow">›</span>
        </button>

        {#if moreVisible}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="ctx-submenu glass"
            class:flip-left={isNearRightEdge}
            class:flip-top={isNearBottomEdge}
            on:mouseenter={keepMore}
            on:mouseleave={hideMoreSubmenu}
          >
            <button class="ctx-item" on:click={() => { previewEntry.set(target); showPreview.set(true); contextMenu.set(null); }}>
              <span class="ctx-icon">◫</span>
              <span class="ctx-label">Preview</span>
              <span class="ctx-shortcut">F3</span>
            </button>
            <button class="ctx-item" on:click={copyPath}>
              <span class="ctx-icon">◈</span>
              <span class="ctx-label">Copy Path</span>
            </button>

            {#if multiSelect}
              <button class="ctx-item" on:click={openBulkRename}>
                <span class="ctx-icon">✏✏</span>
                <span class="ctx-label">Bulk Rename…</span>
              </button>
            {/if}

            {#if targetIsArchive}
              <button class="ctx-item" on:click={extract}>
                <span class="ctx-icon">📦</span>
                <span class="ctx-label">Extract Here</span>
              </button>
            {/if}
            <button class="ctx-item" on:click={openCompress}>
              <span class="ctx-icon">🗜</span>
              <span class="ctx-label">Compress…</span>
            </button>
            {#if !target.is_dir}
              <button class="ctx-item" on:click={showChecksumFor}>
                <span class="ctx-icon">⊛</span>
                <span class="ctx-label">Checksum</span>
              </button>
            {/if}

            <div class="ctx-sep"></div>

            <button class="ctx-item" on:click={openTerminal}>
              <span class="ctx-icon">⬛</span>
              <span class="ctx-label">Open Terminal Here</span>
              <span class="ctx-shortcut">Ctrl+T</span>
            </button>

            {#if contextCmds.length > 0}
              <div class="ctx-sep"></div>
              <div class="ctx-ow-label">Commands</div>
              {#each contextCmds as cmd}
                <button class="ctx-item cmd-item" on:click={() => runCmd(cmd)}>
                  <span class="ctx-icon">{resolveCmdIcon(cmd.icon)}</span>
                  <span class="ctx-label">{cmd.name}</span>
                  {#if cmd.shortcut}
                    <span class="ctx-shortcut">{cmd.shortcut}</span>
                  {/if}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

    {:else}
      <!-- Background right-click -->
      <div class="ctx-label">This Folder</div>
      <div class="ctx-sep"></div>
      <button class="ctx-item" on:click={newFolder}>
        <span class="ctx-icon">📁</span>
        <span class="ctx-label">New Folder</span>
      </button>
      <button class="ctx-item" on:click={newFile}>
        <span class="ctx-icon">📄</span>
        <span class="ctx-label">New File Here</span>
      </button>
      {#if hasClipboard}
        <button class="ctx-item" on:click={paste}>
          <span class="ctx-icon">📋</span>
          <span class="ctx-label">Paste</span>
          <span class="ctx-shortcut">Ctrl+V</span>
        </button>
      {/if}
      <button class="ctx-item" on:click={copyPath}>
        <span class="ctx-icon">◈</span>
        <span class="ctx-label">Copy Path</span>
      </button>
      <button class="ctx-item" on:click={openTerminal}>
        <span class="ctx-icon">⬛</span>
        <span class="ctx-label">Open Terminal Here</span>
      </button>

      <div class="ctx-sep"></div>

      <!-- More options for empty folder -->
      <div
        class="ctx-submenu-wrap"
        on:mouseenter={showMoreSubmenu}
        on:mouseleave={hideMoreSubmenu}
      >
        <button class="ctx-item more-item" on:click={() => moreVisible = !moreVisible}>
          <span class="ctx-icon">⋯</span>
          <span class="ctx-label">Diğer Seçenekler</span>
          <span class="ctx-arrow">›</span>
        </button>

        {#if moreVisible}
          <div
            class="ctx-submenu glass"
            class:flip-left={isNearRightEdge}
            class:flip-top={isNearBottomEdge}
            on:mouseenter={keepMore}
            on:mouseleave={hideMoreSubmenu}
          >
            <button class="ctx-item" on:click={bookmarkCurrent}>
              <span class="ctx-icon">⊹</span>
              <span class="ctx-label">Add to Bookmarks</span>
            </button>

            {#if contextCmds.length > 0}
              <div class="ctx-sep"></div>
              <div class="ctx-ow-label">Commands</div>
              {#each contextCmds as cmd}
                <button class="ctx-item cmd-item" on:click={() => runCmd(cmd)}>
                  <span class="ctx-icon">{resolveCmdIcon(cmd.icon)}</span>
                  <span class="ctx-label">{cmd.name}</span>
                  {#if cmd.shortcut}
                    <span class="ctx-shortcut">{cmd.shortcut}</span>
                  {/if}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

{#if showOpenWith && openWithFile}
  <OpenWithPanel
    filePath={openWithFile.path}
    fileName={openWithFile.name}
    on:close={() => { showOpenWith = false; openWithFile = null; }}
  />
{/if}

<!-- ── Compress dialog ── -->
{#if showCompress}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="mini-overlay" on:click|self={() => showCompress = false}>
    <div class="mini-dialog glass">
      <div class="mini-title">Sıkıştır</div>
      <label class="mini-label">
        Arşiv adı
        <input class="mini-input" bind:value={compressName} placeholder="archive" />
      </label>
      <label class="mini-label">
        Format
        <select class="mini-input" bind:value={compressFormat}>
          <option value="zip">zip</option>
          <option value="tar.gz">tar.gz</option>
          <option value="tar.bz2">tar.bz2</option>
          <option value="tar.xz">tar.xz</option>
          <option value="7z">7z</option>
        </select>
      </label>
      <div class="mini-actions">
        <button class="btn" on:click={() => showCompress = false}>İptal</button>
        <button class="btn btn-accent" on:click={doCompress} disabled={compressBusy || !compressName}>
          {compressBusy ? "..." : "Sıkıştır"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ── Checksum dialog ── -->
{#if showChecksum}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="mini-overlay" on:click|self={() => showChecksum = false}>
    <div class="mini-dialog glass" style="min-width:360px">
      <div class="mini-title">Checksum — {checksumData?.path?.split("/").pop() ?? ""}</div>
      {#if checksumBusy}
        <div class="mini-spin"></div>
      {:else if checksumData}
        <div class="hash-row">
          <span class="hash-label">MD5</span>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span class="hash-val" on:click={() => navigator.clipboard.writeText(checksumData.md5)} title="Kopyala">
            {checksumData.md5}
          </span>
        </div>
        <div class="hash-row">
          <span class="hash-label">SHA256</span>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span class="hash-val" on:click={() => navigator.clipboard.writeText(checksumData.sha256)} title="Kopyala">
            {checksumData.sha256}
          </span>
        </div>
        <div class="mini-hint">Değerlere tıkla → kopyala</div>
      {/if}
      <div class="mini-actions">
        <button class="btn btn-accent" on:click={() => showChecksum = false}>Kapat</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .context-menu {
    position: fixed;
    z-index: 9999;
    min-width: 220px;
    max-height: calc(100vh - 16px);
    overflow: visible;
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 12px 36px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.08);
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    background: var(--bg-panel, rgba(20, 20, 28, 0.94)) !important;
  }
  .ctx-label {
    padding: 4px 10px;
    font-size: 10px;
    color: var(--text-dim);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx-sep { height: 1px; background: var(--border); margin: 3px 0; }
  .ctx-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    border: none;
    background: none;
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    transition: var(--transition-fast);
    user-select: none;
    white-space: nowrap;
    overflow: hidden;
  }
  .ctx-item:hover { background: var(--bg-hover); }
  .ctx-item.danger { color: var(--danger); }
  .ctx-item.danger:hover { background: var(--danger-soft); }
  .ctx-item.more-item {
    font-weight: 500;
    color: var(--text-accent);
  }
  .ctx-icon {
    font-size: 13px;
    min-width: 18px;
    width: auto;
    text-align: center;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .ctx-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ctx-shortcut {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 10px;
    color: var(--text-dim);
    font-family: var(--font-mono);
  }
  .cmd-item { color: var(--text-secondary); }

  /* ── Open With & More Submenu ── */
  .ctx-submenu-wrap {
    position: relative;
  }
  .ctx-arrow {
    margin-left: auto;
    font-size: 13px;
    color: var(--text-dim);
    padding-left: 6px;
  }
  .ctx-submenu {
    position: absolute;
    left: 100%;
    top: -4px;
    min-width: 250px;
    max-width: 330px;
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    overflow-x: hidden;
    border-radius: var(--radius);
    padding: 5px;
    box-shadow: 0 12px 36px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.08);
    z-index: 10001;
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    background: var(--bg-panel, rgba(20, 20, 28, 0.94)) !important;
  }
  .ctx-submenu.flip-left {
    left: auto;
    right: 100%;
  }
  .ctx-submenu.flip-top {
    top: auto;
    bottom: -4px;
  }
  .ctx-ow-label {
    padding: 4px 10px;
    font-size: 10px;
    font-weight: 700;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .ctx-ow-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    font-size: 12.5px;
  }
  .ctx-ow-icon {
    width: 24px;
    height: 24px;
    object-fit: contain;
    border-radius: 4px;
    flex-shrink: 0;
  }
  .ctx-ow-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    font-weight: 500;
  }
  .ctx-ow-badge {
    font-size: 8px;
    padding: 1px 5px;
    background: var(--accent);
    color: #fff;
    border-radius: 3px;
    flex-shrink: 0;
    font-weight: 600;
  }
  .ctx-ow-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px;
  }
  .ctx-ow-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  /* ── Mini dialogs ── */
  .mini-overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.45);
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    display: flex; align-items: center; justify-content: center;
    z-index: 10000;
  }
  .mini-dialog {
    padding: 18px;
    border-radius: var(--radius-lg);
    display: flex; flex-direction: column; gap: 10px;
    min-width: 280px;
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    background: var(--bg-panel, rgba(20, 20, 28, 0.94)) !important;
  }
  .mini-title { font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .mini-label {
    display: flex; flex-direction: column; gap: 4px;
    font-size: 11px; color: var(--text-dim);
  }
  .mini-input {
    padding: 6px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 12px;
  }
  .mini-input:focus { border-color: var(--border-accent); outline: none; }
  .mini-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px; }
  .mini-hint { font-size: 10px; color: var(--text-dim); font-style: italic; }
  .mini-spin {
    width: 20px; height: 20px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    margin: 8px auto;
  }
  .hash-row { display: flex; flex-direction: column; gap: 2px; }
  .hash-label { font-size: 10px; color: var(--text-dim); font-weight: 600; }
  .hash-val {
    font-family: var(--font-mono); font-size: 10px;
    color: var(--text-accent); word-break: break-all;
    cursor: pointer; padding: 4px 6px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
  }
  .hash-val:hover { border-color: var(--accent); }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>