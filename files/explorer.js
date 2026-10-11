import { writable, derived, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";

// ---- Navigation ----
export const currentPath  = writable(localStorage.getItem("lastPath") || "/home");
export const pathHistory  = writable([]);
export const historyIndex = writable(-1);

// ---- Directory contents ----
export const entries   = writable([]);
export const isLoading = writable(false);
export const loadError = writable(null);

// ---- View settings ----
export const viewMode   = writable(localStorage.getItem("viewMode") || "grid");
export const showHidden = writable(localStorage.getItem("showHidden") === "true");
export const sortBy     = writable("name");
export const sortAsc    = writable(true);

// ---- Selection ----
export const selectedPaths = writable(new Set());

// ---- Clipboard ----
// mode: 'copy' | 'cut' | null
export const clipboard = writable({ paths: [], mode: null });

// ---- Search ----
export const searchQuery   = writable("");
export const searchResults = writable([]);
export const isSearching   = writable(false);

// ---- Drives & devices ----
export const drives       = writable([]);
export const blockDevices = writable([]);

// ---- Bookmarks ----
export const bookmarks = writable([]);

// ---- Commands ----
export const userCommands = writable([]);

// ---- Context menu ----
export const contextMenu = writable(null);

// ---- Preview panel ----
export const previewEntry  = writable(null);   // FileEntry | null
export const showPreview   = writable(localStorage.getItem("showPreview") !== "false");
showPreview.subscribe(v => localStorage.setItem("showPreview", String(v)));

// ---- Notifications ----
export const notifications = writable([]);

// ---- Theme ----
export const theme       = writable(localStorage.getItem("theme")   || "dark");
export const opacity     = writable(parseFloat(localStorage.getItem("opacity") || "0.82"));
export const accentColor = writable(localStorage.getItem("accent")  || "#8264ff");

// ---- Persist settings ----
viewMode.subscribe(v   => localStorage.setItem("viewMode", v));
showHidden.subscribe(v => localStorage.setItem("showHidden", String(v)));
theme.subscribe(v => {
  localStorage.setItem("theme", v);
  document.documentElement.setAttribute("data-theme", v);
});
opacity.subscribe(v => {
  localStorage.setItem("opacity", String(v));
  const o = parseFloat(v);
  document.documentElement.style.setProperty("--bg-primary",   `rgba(15,15,20,${o})`);
  document.documentElement.style.setProperty("--bg-secondary", `rgba(22,22,30,${Math.min(o+0.05,1).toFixed(2)})`);
  document.documentElement.style.setProperty("--bg-tertiary",  `rgba(30,30,40,${Math.min(o+0.08,1).toFixed(2)})`);
});
accentColor.subscribe(v => {
  localStorage.setItem("accent", v);
  document.documentElement.style.setProperty("--accent", v);
  const hex = v.replace("#","");
  const r = parseInt(hex.slice(0,2),16);
  const g = parseInt(hex.slice(2,4),16);
  const b = parseInt(hex.slice(4,6),16);
  document.documentElement.style.setProperty("--accent-soft",   `rgba(${r},${g},${b},0.4)`);
  document.documentElement.style.setProperty("--accent-dim",    `rgba(${r},${g},${b},0.15)`);
  document.documentElement.style.setProperty("--border-accent", `rgba(${r},${g},${b},0.35)`);
  document.documentElement.style.setProperty("--text-accent",   v);
  document.documentElement.style.setProperty("--bg-selected",   `rgba(${r},${g},${b},0.25)`);
});

// ---- Navigation ----
export async function loadDirectory(path) {
  isLoading.set(true);
  loadError.set(null);
  searchQuery.set("");
  previewEntry.set(null);
  try {
    const listing = await invoke("list_directory", { path, showHidden: get(showHidden) });
    entries.set(listing.entries);
    currentPath.set(path);
    localStorage.setItem("lastPath", path);
    selectedPaths.set(new Set());
    // inotify: yeni dizini izlemeye başla
    invoke("start_watching", { path }).catch(() => {});
  } catch (e) {
    loadError.set(String(e));
    notify("error", `Cannot open: ${e}`);
  } finally {
    isLoading.set(false);
  }
}

export function navigateTo(path) {
  const hist = get(pathHistory);
  const idx  = get(historyIndex);
  const newHist = [...hist.slice(0, idx + 1), path];
  pathHistory.set(newHist);
  historyIndex.set(newHist.length - 1);
  loadDirectory(path);
}

export function navigateBack() {
  const idx = get(historyIndex);
  if (idx <= 0) return;
  historyIndex.set(idx - 1);
  loadDirectory(get(pathHistory)[idx - 1]);
}

export function navigateForward() {
  const hist = get(pathHistory);
  const idx  = get(historyIndex);
  if (idx >= hist.length - 1) return;
  historyIndex.set(idx + 1);
  loadDirectory(hist[idx + 1]);
}

export function navigateUp() {
  const path = get(currentPath);
  if (path === "/") return;
  const parent = path.split("/").slice(0, -1).join("/") || "/";
  navigateTo(parent);
}

// ---- Clipboard ----
export function copySelected() {
  const paths = [...get(selectedPaths)];
  if (!paths.length) return;
  clipboard.set({ paths, mode: "copy" });
  notify("success", `Copied ${paths.length} item(s)`);
}

export function cutSelected() {
  const paths = [...get(selectedPaths)];
  if (!paths.length) return;
  clipboard.set({ paths, mode: "cut" });
  notify("info", `Cut ${paths.length} item(s) — paste to move`);
}

export async function pasteClipboard() {
  const cb  = get(clipboard);
  const dst = get(currentPath);
  if (!cb.paths.length || !cb.mode) {
    notify("info", "Clipboard is empty");
    return;
  }
  let errors = [];
  for (const src of cb.paths) {
    const name    = src.split("/").pop();
    const dstPath = dst + "/" + name;
    try {
      if (cb.mode === "cut") {
        await invoke("move_item", { src, dst: dstPath });
      } else {
        await invoke("copy_item", { src, dst: dstPath });
      }
    } catch (e) { errors.push(String(e)); }
  }
  if (cb.mode === "cut") clipboard.set({ paths: [], mode: null });
  if (errors.length) notify("error", errors[0]);
  else notify("success", `Pasted ${cb.paths.length} item(s)`);
  await loadDirectory(dst);
}

export function selectAll() {
  const all = new Set(get(entries).map(e => e.path));
  selectedPaths.set(all);
}

// ---- File ops ----
export async function deleteItems(paths, permanent = false) {
  let errors = [];
  for (const p of paths) {
    try {
      await invoke("delete_item", { path: p, permanent });
    } catch (e) { errors.push(`${p}: ${e}`); }
  }
  if (errors.length) notify("error", errors.join("\n"));
  else notify("success", `Deleted ${paths.length} item(s)`);
  await loadDirectory(get(currentPath));
}

// ---- Search ----
export async function searchFiles(query) {
  if (!query.trim()) { searchResults.set([]); return; }
  isSearching.set(true);
  try {
    const results = await invoke("search_files", {
      root: get(currentPath), query, maxResults: 300,
    });
    searchResults.set(results);
  } catch (e) { notify("error", String(e)); }
  finally { isSearching.set(false); }
}

// ---- Bookmarks ----
export async function loadBookmarks() {
  try { bookmarks.set(await invoke("get_bookmarks")); } catch {}
}

export async function addBookmark(name, path) {
  try {
    await invoke("add_bookmark", { name, path });
    await loadBookmarks();
    notify("success", `Bookmarked: ${name}`);
  } catch (e) { notify("error", String(e)); }
}

export async function removeBookmark(path) {
  try {
    await invoke("remove_bookmark", { path });
    await loadBookmarks();
  } catch {}
}

// ---- Drives / Commands ----
export async function loadDrives() {
  try { drives.set(await invoke("get_drives")); } catch {}
}
export async function loadBlockDevices() {
  try { blockDevices.set(await invoke("list_block_devices")); } catch {}
}
export async function loadUserCommands() {
  try { userCommands.set(await invoke("load_user_commands")); } catch {}
}
export async function runCommand(commandId, selectedPath) {
  try {
    const result = await invoke("run_user_command", {
      commandId, selectedPath: selectedPath || null, currentDir: get(currentPath),
    });
    if (result.success) notify("success", "Command completed");
    else notify("error", `Command failed: ${result.stderr}`);
    return result;
  } catch (e) { notify("error", String(e)); throw e; }
}

// ---- Notifications ----
let notifId = 0;
export function notify(type, message, duration = 3500) {
  const id = ++notifId;
  notifications.update(n => [...n, { id, type, message }]);
  setTimeout(() => notifications.update(n => n.filter(x => x.id !== id)), duration);
}

// ---- Derived ----
export const displayEntries = derived(
  [entries, sortBy, sortAsc, searchQuery, searchResults],
  ([$entries, $sortBy, $sortAsc, $searchQuery, $searchResults]) => {
    const list = $searchQuery.trim() ? $searchResults : $entries;
    return [...list].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      switch ($sortBy) {
        case "name":     cmp = a.name.localeCompare(b.name); break;
        case "size":     cmp = a.size - b.size; break;
        case "modified": cmp = a.modified.localeCompare(b.modified); break;
        case "type":     cmp = a.extension.localeCompare(b.extension); break;
      }
      return $sortAsc ? cmp : -cmp;
    });
  }
);
