// stores/shortcuts.js
import { invoke } from "@tauri-apps/api/core";
import { get, writable } from "svelte/store";
import { selectedPaths, currentPath, loadDirectory, showHidden, pasteClipboard, deleteItems, notify } from "./explorer.js";

export const clipboardState = writable(null);
export const lastUndoDesc   = writable("");

// selectedPaths bir Set<string> — path'leri direkt kullanıyoruz
function getSelected() {
    return [...get(selectedPaths)]; // string[] — path listesi
}

const SHORTCUTS = [
    {
        key: "c", ctrl: true, shift: false, alt: false, id: "copy",
        action: async () => {
            const paths = getSelected();
            if (!paths.length) return;
            await invoke("clipboard_copy", { paths });
            clipboardState.set({ op: "copy", paths });
            notify(`${paths.length} dosya kopyalandı`);
        },
    },
    {
        key: "x", ctrl: true, shift: false, alt: false, id: "cut",
        action: async () => {
            const paths = getSelected();
            if (!paths.length) return;
            await invoke("clipboard_cut", { paths });
            clipboardState.set({ op: "cut", paths });
            notify(`${paths.length} dosya kesildi`);
        },
    },
    {
        key: "v", ctrl: true, shift: false, alt: false, id: "paste",
        action: async () => {
            await pasteClipboard();
        },
    },
    {
        key: "z", ctrl: true, shift: false, alt: false, id: "undo",
        action: async () => {
            try {
                const desc = await invoke("undo_last");
                lastUndoDesc.set(desc);
                notify(`Geri alındı: ${desc}`);
                await loadDirectory(get(currentPath));
            } catch (e) {
                notify(String(e) || "Geri alınacak işlem yok");
            }
        },
    },
    {
        key: "Delete", ctrl: false, shift: false, alt: false, id: "delete",
        action: async () => {
            const paths = getSelected();
            if (!paths.length) return;
            await deleteItems(paths, false);
        },
    },
    {
        key: "Delete", ctrl: false, shift: true, alt: false, id: "delete-permanent",
        action: async () => {
            const paths = getSelected();
            if (!paths.length) return;
            if (confirm(`Are you sure you want to permanently delete ${paths.length} selected item(s)?`)) {
                await deleteItems(paths, true);
            }
        },
    },
    {
        key: "F5", ctrl: false, shift: false, alt: false, id: "refresh",
        action: async () => { await loadDirectory(get(currentPath)); },
    },
    {
        key: "F7", ctrl: false, shift: false, alt: false, id: "new-folder",
        action: async () => {
            const name = prompt("Folder name:");
            if (!name) return;
            const path = `${get(currentPath)}/${name}`;
            await invoke("tracked_create_dir", { path });
            await loadDirectory(get(currentPath));
        },
    },
    {
        key: "n", ctrl: true, shift: true, alt: false, id: "new-file",
        action: async () => {
            const name = prompt("Dosya adı:");
            if (!name) return;
            const path = `${get(currentPath)}/${name}`;
            await invoke("tracked_create_file", { path });
            await loadDirectory(get(currentPath));
        },
    },
    {
        key: "a", ctrl: true, shift: false, alt: false, id: "select-all",
        action: () => { window.dispatchEvent(new CustomEvent("arch:select-all")); },
    },
    {
        key: "ArrowLeft", ctrl: false, shift: false, alt: true, id: "nav-back",
        action: () => import("./explorer.js").then(m => m.navigateBack()),
    },
    {
        key: "ArrowRight", ctrl: false, shift: false, alt: true, id: "nav-forward",
        action: () => import("./explorer.js").then(m => m.navigateForward()),
    },
    {
        key: "ArrowUp", ctrl: false, shift: false, alt: true, id: "nav-up",
        action: () => import("./explorer.js").then(m => m.navigateUp()),
    },
    {
        key: "h", ctrl: true, shift: false, alt: false, id: "toggle-hidden",
        action: () => {
            showHidden.update(v => !v);
            loadDirectory(get(currentPath));
        },
    },
    {
        key: "F2", ctrl: false, shift: false, alt: false, id: "rename",
        action: () => { window.dispatchEvent(new CustomEvent("arch:rename-selected")); },
    },
    {
        key: "Enter", ctrl: false, shift: false, alt: true, id: "properties",
        action: () => { window.dispatchEvent(new CustomEvent("arch:show-properties")); },
    },
];

let _registered = false;

export function registerShortcuts() {
    if (_registered) return;
    _registered = true;
    window.addEventListener("keydown", handleKeyDown);
}

export function unregisterShortcuts() {
    window.removeEventListener("keydown", handleKeyDown);
    _registered = false;
}

function handleKeyDown(e) {
    const tag = document.activeElement?.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea") return;

    for (const sc of SHORTCUTS) {
        if (
            e.key === sc.key &&
            !!e.ctrlKey  === sc.ctrl &&
            !!e.shiftKey === sc.shift &&
            !!e.altKey   === sc.alt
        ) {
            e.preventDefault();
            e.stopPropagation();
            sc.action();
            return;
        }
    }
}

export function getShortcutLabel(id) {
    const sc = SHORTCUTS.find(s => s.id === id);
    if (!sc) return "";
    const parts = [];
    if (sc.ctrl)  parts.push("Ctrl");
    if (sc.shift) parts.push("Shift");
    if (sc.alt)   parts.push("Alt");
    parts.push(sc.key.length === 1 ? sc.key.toUpperCase() : sc.key);
    return parts.join("+");
}

// Explorer'ın notify sistemini kullan — Tauri notification plugin'e dokunma
function notify(msg) {
    window.dispatchEvent(new CustomEvent("arch:notify", {
        detail: { message: msg, type: "success" }
    }));
}

function notifyError(msg) {
    window.dispatchEvent(new CustomEvent("arch:notify", {
        detail: { message: String(msg), type: "error" }
    }));
}
