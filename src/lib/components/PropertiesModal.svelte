<script>
  import { onMount, createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { notify, requestSudo } from "../stores/explorer.js";

  export let filePath = "";

  const dispatch = createEventDispatcher();

  let loading = true;
  let error = null;
  let props = null;

  // Dir size calculation
  let calculatingDir = false;
  let dirSizeResult = null;

  // Checksum
  let calculatingHash = false;
  let checksumResult = null;

  // Edit Permissions & Ownership
  let isEditingPerms = false;
  let editOctal = "755";
  let editRwx = "rwxr-xr-x";
  let isEditingOwner = false;
  let editOwner = "";
  let editGroup = "";
  let applyRecursive = false;
  let busySaving = false;

  // System users & groups
  let systemUsers = [];
  let systemGroups = [];

  function octalToRwx(oct) {
    const s = (oct || "").trim().padStart(3, "0").slice(-3);
    const m = {
      "0": "---", "1": "--x", "2": "-w-", "3": "-wx",
      "4": "r--", "5": "r-x", "6": "rw-", "7": "rwx"
    };
    return (m[s[0]] || "---") + (m[s[1]] || "---") + (m[s[2]] || "---");
  }

  function rwxToOctal(rwx) {
    const s = (rwx || "").padEnd(9, "-").slice(0, 9);
    let oct = "";
    for (let i = 0; i < 9; i += 3) {
      let v = 0;
      if (s[i] === "r") v += 4;
      if (s[i + 1] === "w") v += 2;
      if (s[i + 2] === "x") v += 1;
      oct += v.toString();
    }
    return oct;
  }

  function applyPreset(oct) {
    editOctal = oct;
    editRwx = octalToRwx(oct);
  }

  function onOctalInput(val) {
    editOctal = val.replace(/[^0-7]/g, "").slice(0, 3);
    if (editOctal.length === 3) {
      editRwx = octalToRwx(editOctal);
    }
  }

  function onRwxInput(val) {
    // Sadece r, w, x, - karakterlerine izin ver
    let clean = "";
    for (const c of val.toLowerCase()) {
      if (["r", "w", "x", "-"].includes(c)) clean += c;
    }
    editRwx = clean.slice(0, 9);
    if (editRwx.length === 9) {
      editOctal = rwxToOctal(editRwx);
    }
  }

  function togglePermBit(index, char) {
    let arr = editRwx.padEnd(9, "-").split("");
    if (arr[index] === char) {
      arr[index] = "-";
    } else {
      arr[index] = char;
    }
    editRwx = arr.join("");
    editOctal = rwxToOctal(editRwx);
  }

  function explainPermissions(oct, isDir) {
    const s = (oct || "").trim().padStart(3, "0").slice(-3);
    function exp(char, role) {
      const v = parseInt(char, 10) || 0;
      const acts = [];
      if (v & 4) acts.push("Okuma (r)");
      if (v & 2) acts.push("Yazma / Değiştirme (w)");
      if (v & 1) acts.push(isDir ? "Klasöre Giriş (x)" : "Çalıştırma (x)");
      if (acts.length === 0) return `${role}: Yetki Yok (—)`;
      return `${role}: ${acts.join(", ")}`;
    }
    return [
      exp(s[0], "Sahip (User)"),
      exp(s[1], "Grup (Group)"),
      exp(s[2], "Diğerleri (Others)")
    ];
  }

  $: currentExpl = explainPermissions(editOctal, props?.is_dir);

  function close() {
    dispatch("close");
  }

  function handleKeydown(e) {
    if (e.key === "Escape") {
      close();
    }
  }

  function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return (bytes / Math.pow(k, i)).toFixed(2) + " " + sizes[i];
  }

  async function loadProperties() {
    loading = true;
    error = null;
    try {
      props = await invoke("get_file_properties", { path: filePath });
      if (props) {
        editOctal = props.permissions_octal?.slice(-3) || "755";
        editRwx = octalToRwx(editOctal);
        editOwner = props.owner_name || "";
        editGroup = props.group_name || "";
      }
      // Sistem kullanıcıları ve gruplarını yükle
      invoke("get_system_users_and_groups")
        .then(acc => {
          if (acc) {
            systemUsers = acc.users || [];
            systemGroups = acc.groups || [];
          }
        })
        .catch(() => {});
      if (props?.is_dir) {
        // Otomatik cache'ten klasör boyutunu yokla
        invoke("get_dir_size", { path: filePath })
          .then(res => {
            if (res && res.size > 0) dirSizeResult = res;
          })
          .catch(() => {});
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function calculateDirSize() {
    calculatingDir = true;
    try {
      const res = await invoke("get_dir_size", { path: filePath });
      dirSizeResult = res;
    } catch (e) {
      notify("error", `Boyut hesaplanamadı: ${e}`);
    } finally {
      calculatingDir = false;
    }
  }

  async function calculateChecksum() {
    calculatingHash = true;
    try {
      const res = await invoke("get_checksum", { path: filePath });
      checksumResult = res;
    } catch (e) {
      notify("error", `Checksum alınamadı: ${e}`);
    } finally {
      calculatingHash = false;
    }
  }

  function copyText(text, label = "Kopyalandı") {
    navigator.clipboard.writeText(text);
    notify("success", `${label}`);
  }

  async function savePermissions() {
    const octVal = parseInt(editOctal, 8);
    if (isNaN(octVal)) {
      notify("error", "Geçersiz oktal izin (örnek: 755 veya 644)");
      return;
    }
    busySaving = true;
    try {
      await invoke("set_file_permissions", {
        path: filePath,
        mode: octVal,
        recursive: applyRecursive
      });
      notify("success", "İzinler güncellendi");
      isEditingPerms = false;
      await loadProperties();
    } catch (err) {
      const errStr = String(err).toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("yetki") || errStr.includes("operation not permitted")) {
        try {
          await requestSudo(
            "İzin Değişikliği İçin Root Yetkisi",
            `'${filePath}' izinlerini değiştirmek için yönetici şifresi gereklidir.`,
            async (password) => {
              await invoke("sudo_set_file_permissions", {
                path: filePath,
                mode: octVal,
                recursive: applyRecursive,
                password
              });
            }
          );
          notify("success", "İzinler root yetkisiyle güncellendi");
          isEditingPerms = false;
          await loadProperties();
        } catch (sudoErr) {
          notify("error", String(sudoErr));
        }
      } else {
        notify("error", String(err));
      }
    } finally {
      busySaving = false;
    }
  }

  async function saveOwner() {
    if (!editOwner.trim()) {
      notify("error", "Sahip kullanıcı adı boş olamaz");
      return;
    }
    busySaving = true;
    try {
      await invoke("set_file_owner", {
        path: filePath,
        owner: editOwner.trim(),
        group: editGroup.trim(),
        recursive: applyRecursive
      });
      notify("success", "Sahip/Grup güncellendi");
      isEditingOwner = false;
      await loadProperties();
    } catch (err) {
      const errStr = String(err).toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("yetki") || errStr.includes("operation not permitted")) {
        try {
          await requestSudo(
            "Sahiplik Değişikliği İçin Root Yetkisi",
            `'${filePath}' sahipliğini değiştirmek için yönetici şifresi gereklidir.`,
            async (password) => {
              await invoke("sudo_set_file_owner", {
                path: filePath,
                owner: editOwner.trim(),
                group: editGroup.trim(),
                recursive: applyRecursive,
                password
              });
            }
          );
          notify("success", "Sahiplik root yetkisiyle güncellendi");
          isEditingOwner = false;
          await loadProperties();
        } catch (sudoErr) {
          notify("error", String(sudoErr));
        }
      } else {
        notify("error", String(err));
      }
    } finally {
      busySaving = false;
    }
  }

  onMount(() => {
    if (filePath) loadProperties();
  });
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="prop-backdrop" on:click|self={close}>
  <div class="prop-dialog glass">
    <div class="prop-header">
      <div class="prop-title-row">
        <span class="prop-icon">{props?.is_dir ? "📁" : (props?.is_symlink ? "⤷" : "📄")}</span>
        <span class="prop-title" title={props?.name || filePath}>{props?.name || "Özellikler"}</span>
      </div>
      <button class="prop-close-btn" on:click={close} title="Kapat (Esc)">✕</button>
    </div>

    {#if loading}
      <div class="prop-loading">
        <div class="spinner"></div>
        <span>Bilgiler alınıyor...</span>
      </div>
    {:else if error}
      <div class="prop-error">
        <span>⚠</span>
        <span>{error}</span>
      </div>
    {:else if props}
      <div class="prop-body">
        <!-- Genel Bilgiler -->
        <div class="prop-section">
          <div class="prop-row">
            <span class="pk">Dosya Adı:</span>
            <span class="pv selectable" title={props.name}>{props.name}</span>
          </div>

          <div class="prop-row">
            <span class="pk">Tür:</span>
            <span class="pv">
              {#if props.is_dir}
                Klasör (Dizin)
              {:else if props.is_symlink}
                Sembolik Bağ ({props.symlink_target || "bağlantı"})
              {:else}
                {props.extension ? props.extension.toUpperCase() + " Dosyası" : "Dosya"} ({props.mime_type})
              {/if}
            </span>
          </div>

          <div class="prop-row">
            <span class="pk">Konum:</span>
            <div class="pv-with-action">
              <span class="pv mono path-text" title={props.parent_dir}>{props.parent_dir}</span>
              <button class="mini-btn" on:click={() => copyText(props.parent_dir, "Konum kopyalandı")} title="Konumu Kopyala">📋</button>
            </div>
          </div>

          <div class="prop-row">
            <span class="pk">Tam Yol:</span>
            <div class="pv-with-action">
              <span class="pv mono path-text" title={props.path}>{props.path}</span>
              <button class="mini-btn" on:click={() => copyText(props.path, "Yol kopyalandı")} title="Yolu Kopyala">📋</button>
            </div>
          </div>
        </div>

        <div class="prop-divider"></div>

        <!-- Boyut & İçerik -->
        <div class="prop-section">
          <div class="prop-row">
            <span class="pk">Boyut:</span>
            <div class="pv-with-action">
              {#if props.is_dir}
                {#if dirSizeResult && dirSizeResult.size > 0}
                  <span class="pv font-bold">{formatBytes(dirSizeResult.size)}</span>
                  <span class="pv-sub mono">({dirSizeResult.size.toLocaleString()} bayt)</span>
                {:else if calculatingDir}
                  <span class="pv-sub" style="color:var(--accent)">Hesaplanıyor…</span>
                {:else}
                  <span class="pv-sub">—</span>
                  <button class="calc-btn" on:click={calculateDirSize} disabled={calculatingDir}>Hesapla</button>
                {/if}
              {:else}
                <span class="pv font-bold">{formatBytes(props.size)}</span>
                <span class="pv-sub mono">({props.size.toLocaleString()} bayt)</span>
              {/if}
            </div>
          </div>

          {#if props.is_dir}
            <div class="prop-row">
              <span class="pk">İçerik:</span>
              <span class="pv">
                {#if dirSizeResult && dirSizeResult.file_count > 0}
                  {dirSizeResult.file_count.toLocaleString()} öğe (alt dizinler dahil)
                {:else if props.file_count !== null}
                  {props.file_count} öğe (bu dizinde)
                {:else}
                  —
                {/if}
              </span>
            </div>
          {/if}
        </div>

        <div class="prop-divider"></div>

        <!-- Zaman Damgaları -->
        <div class="prop-section">
          <div class="prop-row">
            <span class="pk">Değiştirilme:</span>
            <span class="pv mono">{props.modified}</span>
          </div>
          <div class="prop-row">
            <span class="pk">Oluşturulma:</span>
            <span class="pv mono">{props.created}</span>
          </div>
          <div class="prop-row">
            <span class="pk">Son Erişim:</span>
            <span class="pv mono">{props.accessed}</span>
          </div>
        </div>

        <div class="prop-divider"></div>

        <!-- İzinler ve Sahiplik -->
        <div class="prop-section">
          <div class="prop-row">
            <span class="pk">İzinler:</span>
            <div class="pv-with-action" style="flex: 1;">
              {#if !isEditingPerms}
                <span class="pv mono perm-tag selectable">{props.permissions_str}</span>
                <span class="pv mono octal-tag selectable">({props.permissions_octal})</span>
                {#if props.is_readonly}
                  <span class="readonly-badge">Yalnızca Okunur</span>
                {/if}
                <button class="mini-btn edit-btn" on:click={() => { isEditingPerms = true; editOctal = props.permissions_octal?.slice(-3) || "755"; editRwx = octalToRwx(editOctal); }} title="İzinleri Değiştir">✏ Değiştir</button>
              {:else}
                <div class="edit-box-rich">
                  <!-- Sayısal (Oktal) ve Harfsel (RWX) Çift Yönlü Giriş + Hızlı Önayarlar -->
                  <div class="perm-top-row">
                    <div class="input-labeled">
                      <span class="edit-lbl">Oktal:</span>
                      <input
                        class="mini-input mono perm-num-input"
                        type="text"
                        maxlength="3"
                        value={editOctal}
                        on:input={e => onOctalInput(e.target.value)}
                        placeholder="755"
                      />
                    </div>
                    <div class="input-labeled" style="flex:1">
                      <span class="edit-lbl">RWX:</span>
                      <input
                        class="mini-input mono perm-rwx-input"
                        type="text"
                        maxlength="9"
                        value={editRwx}
                        on:input={e => onRwxInput(e.target.value)}
                        placeholder="rwxr-xr-x"
                      />
                    </div>
                  </div>

                  <!-- Hızlı Hazır Ayarlar (Presets) -->
                  <div class="perm-presets">
                    <button type="button" class="preset-chip" class:active={editOctal === "755"} on:click={() => applyPreset("755")}>755 (Standart)</button>
                    <button type="button" class="preset-chip" class:active={editOctal === "644"} on:click={() => applyPreset("644")}>644 (Dosya)</button>
                    <button type="button" class="preset-chip" class:active={editOctal === "700"} on:click={() => applyPreset("700")}>700 (Özel)</button>
                    <button type="button" class="preset-chip" class:active={editOctal === "777"} on:click={() => applyPreset("777")}>777 (Tam Yetki)</button>
                  </div>

                  <!-- Kolay Seçim Butonları / Tikler (Sahip, Grup, Diğerleri) -->
                  <div class="perm-matrix">
                    <div class="matrix-col">
                      <span class="matrix-title">👤 Sahip</span>
                      <div class="matrix-btns">
                        <button type="button" class="bit-btn" class:active={editRwx[0] === 'r'} on:click={() => togglePermBit(0, 'r')}>r</button>
                        <button type="button" class="bit-btn" class:active={editRwx[1] === 'w'} on:click={() => togglePermBit(1, 'w')}>w</button>
                        <button type="button" class="bit-btn" class:active={editRwx[2] === 'x'} on:click={() => togglePermBit(2, 'x')}>x</button>
                      </div>
                    </div>
                    <div class="matrix-col">
                      <span class="matrix-title">👥 Grup</span>
                      <div class="matrix-btns">
                        <button type="button" class="bit-btn" class:active={editRwx[3] === 'r'} on:click={() => togglePermBit(3, 'r')}>r</button>
                        <button type="button" class="bit-btn" class:active={editRwx[4] === 'w'} on:click={() => togglePermBit(4, 'w')}>w</button>
                        <button type="button" class="bit-btn" class:active={editRwx[5] === 'x'} on:click={() => togglePermBit(5, 'x')}>x</button>
                      </div>
                    </div>
                    <div class="matrix-col">
                      <span class="matrix-title">🌐 Diğer</span>
                      <div class="matrix-btns">
                        <button type="button" class="bit-btn" class:active={editRwx[6] === 'r'} on:click={() => togglePermBit(6, 'r')}>r</button>
                        <button type="button" class="bit-btn" class:active={editRwx[7] === 'w'} on:click={() => togglePermBit(7, 'w')}>w</button>
                        <button type="button" class="bit-btn" class:active={editRwx[8] === 'x'} on:click={() => togglePermBit(8, 'x')}>x</button>
                      </div>
                    </div>
                  </div>

                  <!-- Anlaşılır Açıklama Özeti -->
                  <div class="perm-summary-box">
                    <div class="summary-line"><strong>👤</strong> {currentExpl[0]}</div>
                    <div class="summary-line"><strong>👥</strong> {currentExpl[1]}</div>
                    <div class="summary-line"><strong>🌐</strong> {currentExpl[2]}</div>
                  </div>

                  {#if props.is_dir}
                    <label class="rec-label" style="margin-top: 4px;">
                      <input type="checkbox" bind:checked={applyRecursive} /> Alt dizin ve dosyalara da uygula (-R)
                    </label>
                  {/if}

                  <div class="edit-actions" style="margin-top: 6px;">
                    <button class="mini-btn save-btn" on:click={savePermissions} disabled={busySaving}>
                      {busySaving ? "..." : "✓ İzinleri Kaydet"}
                    </button>
                    <button class="mini-btn cancel-btn" on:click={() => isEditingPerms = false} disabled={busySaving}>✕ İptal</button>
                  </div>
                </div>
              {/if}
            </div>
          </div>

          <div class="prop-row">
            <span class="pk">Sahip / Grup:</span>
            <div class="pv-with-action" style="flex: 1;">
              {#if !isEditingOwner}
                <span class="pv mono selectable">
                  <strong>{props.owner_name}</strong> : <strong>{props.group_name}</strong>
                  <span class="pv-sub mono">({props.owner_uid}:{props.owner_gid})</span>
                </span>
                <button class="mini-btn edit-btn" on:click={() => { isEditingOwner = true; editOwner = props.owner_name; editGroup = props.group_name; }} title="Sahip/Grup Değiştir">✏ Değiştir</button>
              {:else}
                <div class="edit-box-rich">
                  <div class="owner-inputs-row">
                    <div class="input-labeled" style="flex: 1; min-width: 0;">
                      <span class="edit-lbl">Kullanıcı (Sahip):</span>
                      <input
                        class="mini-input mono unified-combo"
                        type="text"
                        bind:value={editOwner}
                        list="users-datalist"
                        placeholder="kullanıcı seç/yaz"
                      />
                      <datalist id="users-datalist">
                        {#each systemUsers as u}
                          <option value={u}>{u}</option>
                        {/each}
                      </datalist>
                    </div>

                    <div class="input-labeled" style="flex: 1; min-width: 0;">
                      <span class="edit-lbl">Grup:</span>
                      <input
                        class="mini-input mono unified-combo"
                        type="text"
                        bind:value={editGroup}
                        list="groups-datalist"
                        placeholder="grup seç/yaz"
                      />
                      <datalist id="groups-datalist">
                        {#each systemGroups as g}
                          <option value={g}>{g}</option>
                        {/each}
                      </datalist>
                    </div>
                  </div>

                  <p class="hint" style="margin-top: 6px;">
                    Listeden seçebilir veya elle yazabilirsiniz. Başka bir kullanıcıya devretmek için root (sudo) yetkisi otomatik istenecektir.
                  </p>

                  {#if props.is_dir}
                    <label class="rec-label" style="margin-top: 4px;">
                      <input type="checkbox" bind:checked={applyRecursive} /> Alt dizin ve dosyalara da uygula (-R)
                    </label>
                  {/if}

                  <div class="edit-actions" style="margin-top: 8px;">
                    <button class="mini-btn save-btn" on:click={saveOwner} disabled={busySaving}>
                      {busySaving ? "..." : "✓ Sahipliği Kaydet"}
                    </button>
                    <button class="mini-btn cancel-btn" on:click={() => isEditingOwner = false} disabled={busySaving}>✕ İptal</button>
                  </div>
                </div>
              {/if}
            </div>
          </div>
        </div>

        <!-- Checksum (Sadece dosyalar için) -->
        {#if !props.is_dir}
          <div class="prop-divider"></div>
          <div class="prop-section">
            <div class="prop-row">
              <span class="pk">Checksum:</span>
              <div class="pv-with-action" style="flex:1">
                {#if checksumResult}
                  <div class="hash-box">
                    <div class="hash-row">
                      <span class="hash-label">MD5:</span>
                      <span class="hash-val mono" title={checksumResult.md5}>{checksumResult.md5}</span>
                      <button class="mini-btn" on:click={() => copyText(checksumResult.md5, "MD5 kopyalandı")}>📋</button>
                    </div>
                    <div class="hash-row">
                      <span class="hash-label">SHA256:</span>
                      <span class="hash-val mono" title={checksumResult.sha256}>{checksumResult.sha256}</span>
                      <button class="mini-btn" on:click={() => copyText(checksumResult.sha256, "SHA256 kopyalandı")}>📋</button>
                    </div>
                  </div>
                {:else if calculatingHash}
                  <span class="pv-sub" style="color:var(--accent)">Hesaplanıyor…</span>
                {:else}
                  <button class="calc-btn" on:click={calculateChecksum} disabled={calculatingHash}>
                    MD5 / SHA256 Doğrula
                  </button>
                {/if}
              </div>
            </div>
          </div>
        {/if}
      </div>

      <div class="prop-footer">
        <button class="btn btn-accent" on:click={close}>Tamam</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .prop-backdrop {
    position: fixed;
    inset: 0;
    z-index: 999;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    
    
  }

  .prop-dialog {
    width: 440px;
    max-width: 95vw;
    max-height: 88vh;
    background: var(--bg-secondary, rgba(22, 22, 30, 0.95));
    border: 1px solid var(--border, rgba(255, 255, 255, 0.12));
    border-radius: var(--radius-lg, 12px);
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .prop-header {
    height: 48px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    flex-shrink: 0;
  }

  .prop-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1;
  }

  .prop-icon {
    font-size: 20px;
    flex-shrink: 0;
  }

  .prop-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary, #fff);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .prop-close-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--text-dim, rgba(255, 255, 255, 0.4));
    font-size: 14px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .prop-close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .prop-loading, .prop-error {
    padding: 40px 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-dim, rgba(255, 255, 255, 0.5));
    font-size: 12px;
  }
  .prop-error { color: var(--danger, #ff5f6d); }

  .prop-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .prop-section {
    display: flex;
    flex-direction: column;
    gap: 7px;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .prop-divider {
    height: 1px;
    background: var(--border, rgba(255, 255, 255, 0.08));
    margin: 2px 0;
  }

  .prop-row {
    display: flex;
    align-items: flex-start;
    font-size: 11.5px;
    line-height: 1.45;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .pk {
    width: 95px;
    flex-shrink: 0;
    color: var(--text-dim, rgba(255, 255, 255, 0.4));
    font-weight: 500;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .pv {
    flex: 1;
    color: var(--text-primary, rgba(255, 255, 255, 0.85));
    word-break: break-all;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .pv.selectable, .selectable {
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .pv-sub {
    color: var(--text-dim, rgba(255, 255, 255, 0.45));
    font-size: 10.5px;
    margin-left: 4px;
  }

  .font-bold { font-weight: 600; }
  .mono { font-family: var(--font-mono, monospace); font-size: 11px; }

  .pv-with-action {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .path-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .mini-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-secondary, #ddd);
    font-size: 11px;
    padding: 2px 5px;
    border-radius: 4px;
    cursor: pointer;
    flex-shrink: 0;
  }
  .mini-btn:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .edit-btn {
    font-size: 10px;
    padding: 2px 7px;
    background: var(--accent-dim, rgba(130, 100, 255, 0.15));
    border-color: var(--border-accent, rgba(130, 100, 255, 0.35));
    color: var(--text-accent, #a892ff);
  }
  .edit-btn:hover {
    background: var(--accent-soft, rgba(130, 100, 255, 0.3));
    color: #fff;
  }

  .edit-box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border-accent, rgba(130, 100, 255, 0.3));
    padding: 6px 8px;
    border-radius: 6px;
    width: 100%;
  }

  .edit-box-rich {
    display: flex;
    flex-direction: column;
    gap: 7px;
    background: rgba(0, 0, 0, 0.32);
    border: 1px solid var(--border-accent, rgba(130, 100, 255, 0.35));
    padding: 10px;
    border-radius: 8px;
    width: 100%;
  }

  .perm-top-row, .owner-inputs-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .perm-presets {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .preset-chip {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 4px;
    color: var(--text-secondary, #bbb);
    font-size: 10px;
    padding: 3px 8px;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .preset-chip:hover {
    background: rgba(130, 100, 255, 0.15);
    border-color: var(--accent, #8264ff);
    color: #fff;
  }
  .preset-chip.active {
    background: var(--accent, #8264ff);
    border-color: var(--accent, #8264ff);
    color: #fff;
    font-weight: 600;
  }

  .unified-combo {
    width: 100%;
    box-sizing: border-box;
  }

  .input-labeled {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .perm-num-input {
    width: 65px;
    text-align: center;
    font-weight: 700;
    font-size: 13px;
    letter-spacing: 0.1em;
    color: var(--accent, #a892ff);
  }

  .perm-rwx-input {
    letter-spacing: 0.08em;
    font-weight: 600;
  }

  .perm-matrix {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    background: rgba(255, 255, 255, 0.03);
    padding: 6px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .matrix-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .matrix-title {
    font-size: 9.5px;
    color: var(--text-dim, #888);
    font-weight: 600;
  }

  .matrix-btns {
    display: flex;
    gap: 3px;
  }

  .bit-btn {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(0, 0, 0, 0.3);
    color: var(--text-dim, #777);
    font-family: var(--font-mono, monospace);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.12s ease;
  }

  .bit-btn.active {
    background: var(--accent, #8264ff);
    color: #fff;
    border-color: var(--accent, #8264ff);
    box-shadow: 0 0 8px rgba(130, 100, 255, 0.4);
  }

  .perm-tips {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 10px;
    color: var(--text-dim, #999);
    background: rgba(0, 0, 0, 0.2);
    padding: 5px 8px;
    border-radius: 5px;
  }

  .tip-key {
    color: var(--accent, #8264ff);
    font-weight: 700;
    font-family: var(--font-mono, monospace);
  }

  .perm-summary-box {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 10.5px;
    background: rgba(130, 100, 255, 0.08);
    border: 1px solid rgba(130, 100, 255, 0.2);
    padding: 6px 8px;
    border-radius: 6px;
    color: var(--text-primary, #ddd);
  }

  .summary-line strong {
    color: var(--accent, #8264ff);
  }

  .combo-wrap {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .combo-input {
    flex: 1;
    min-width: 0;
  }

  .combo-select {
    width: 60px;
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    color: var(--text-secondary, #ccc);
    font-size: 10px;
    padding: 2px 2px;
    cursor: pointer;
    outline: none;
  }
  .combo-select:focus {
    border-color: var(--accent, #8264ff);
  }

  .hint {
    font-size: 10px;
    color: var(--text-dim, #888);
    margin: 0;
    line-height: 1.35;
  }

  .edit-inputs {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .edit-lbl {
    font-size: 10.5px;
    color: var(--text-dim, rgba(255, 255, 255, 0.5));
  }

  .mini-input {
    background: rgba(0, 0, 0, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    color: #fff;
    padding: 2px 6px;
    font-size: 11px;
    outline: none;
  }
  .mini-input:focus {
    border-color: var(--accent, #8264ff);
  }

  .rec-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    color: var(--text-secondary, #ccc);
    cursor: pointer;
    margin-left: 6px;
  }

  .edit-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .save-btn {
    background: var(--accent, #8264ff);
    color: #fff;
    border: none;
    padding: 3px 10px;
    font-weight: 500;
  }
  .save-btn:hover {
    filter: brightness(1.15);
  }

  .cancel-btn {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-dim, #aaa);
    padding: 3px 8px;
  }

  .selectable {
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .calc-btn {
    background: var(--accent-dim, rgba(130, 100, 255, 0.15));
    border: 1px solid var(--border-accent, rgba(130, 100, 255, 0.35));
    color: var(--text-accent, #8264ff);
    font-size: 10.5px;
    padding: 2px 8px;
    border-radius: 4px;
    cursor: pointer;
  }
  .calc-btn:hover:not(:disabled) {
    background: var(--accent, #8264ff);
    color: #fff;
  }

  .perm-tag {
    background: rgba(0, 0, 0, 0.3);
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .octal-tag {
    color: var(--accent, #8264ff);
    font-weight: 600;
  }

  .readonly-badge {
    background: rgba(255, 95, 109, 0.15);
    border: 1px solid rgba(255, 95, 109, 0.35);
    color: var(--danger, #ff5f6d);
    font-size: 9.5px;
    padding: 1px 6px;
    border-radius: 4px;
    margin-left: 6px;
  }

  .hash-box {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: rgba(0, 0, 0, 0.25);
    padding: 8px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .hash-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .hash-label {
    width: 55px;
    font-size: 9.5px;
    color: var(--text-dim, rgba(255, 255, 255, 0.4));
    font-weight: 600;
  }

  .hash-val {
    flex: 1;
    font-size: 9.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary, rgba(255, 255, 255, 0.75));
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .prop-footer {
    height: 48px;
    padding: 0 16px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    background: rgba(0, 0, 0, 0.15);
    flex-shrink: 0;
  }

  .btn {
    padding: 6px 18px;
    font-size: 12px;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 500;
    border: none;
  }
  .btn-accent {
    background: var(--accent, #8264ff);
    color: #fff;
  }
  .btn-accent:hover {
    filter: brightness(1.1);
  }
</style>
