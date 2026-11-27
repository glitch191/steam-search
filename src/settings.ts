import { invoke } from "@tauri-apps/api/core";
import { hotkeyFromEvent, needsModifier } from "./hotkey";
import "./settings.css";

/** Settings as returned by the backend. Field names match config.json. */
interface Settings {
  hotkey: string;
  maxResults: number;
  steamPath: string;
  startWithWindows: boolean;
  fontFamily: string;
  fontSize: number;
  steamFoundAt: string;
  sampleGames: string[];
}

const DEFAULT_FONT_LABEL = "Default (Segoe UI)";
const DEFAULT_FONT_SIZE = 16;
const SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 32, 36, 40];
const FALLBACK_SAMPLES = ["Example game", "Another game title", "A game with a much longer name than the others"];

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const startWithWindows = $<HTMLInputElement>("start-with-windows");
const hotkeyField = $<HTMLInputElement>("hotkey");
const hotkeyChange = $<HTMLButtonElement>("hotkey-change");
const hotkeyHint = $<HTMLParagraphElement>("hotkey-hint");
const maxResults = $<HTMLInputElement>("max-results");
const steamPath = $<HTMLInputElement>("steam-path");
const steamStatus = $<HTMLParagraphElement>("steam-status");
const familyInput = $<HTMLInputElement>("family-input");
const familyList = $<HTMLUListElement>("family-list");
const sizeInput = $<HTMLInputElement>("size-input");
const sizeList = $<HTMLUListElement>("size-list");
const preview = $<HTMLUListElement>("preview");
const fontReset = $<HTMLButtonElement>("font-reset");

const HOTKEY_HINT = hotkeyHint.textContent ?? "";

let settings: Settings;
/** Installed font families; "" stands for the default font. */
let families: string[] = [""];
let capturing = false;
let fontSaveTimer: number | undefined;

// ---------- Saving ----------

async function save(patch: Partial<Settings>): Promise<void> {
  settings = await invoke<Settings>("save_settings", { patch });
  showGeneral();
}

// ---------- Tabs ----------

const tabs = Array.from(document.querySelectorAll<HTMLButtonElement>('[role="tab"]'));

function selectTab(tab: HTMLButtonElement): void {
  for (const t of tabs) {
    const active = t === tab;
    t.setAttribute("aria-selected", String(active));
    t.tabIndex = active ? 0 : -1;
    $(t.getAttribute("aria-controls")!).hidden = !active;
  }
  if (tab.id === "tab-font") scrollSelectedIntoView();
}

for (const tab of tabs) {
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const next = tabs[(tabs.indexOf(tab) + (e.key === "ArrowDown" ? 1 : -1) + tabs.length) % tabs.length];
    next.focus();
    selectTab(next);
  });
}

// ---------- General ----------

function showGeneral(): void {
  startWithWindows.checked = settings.startWithWindows;
  if (!capturing) hotkeyField.value = settings.hotkey;
  maxResults.value = String(settings.maxResults);
  if (document.activeElement !== steamPath) steamPath.value = settings.steamPath;
  steamStatus.classList.toggle("error", !settings.steamFoundAt);
  if (settings.steamFoundAt) {
    steamStatus.textContent = settings.steamPath
      ? `Steam found in this folder.`
      : `Detected automatically: ${settings.steamFoundAt}. Fill this in only if detection picks the wrong folder.`;
  } else {
    steamStatus.textContent = settings.steamPath
      ? "No Steam installation in this folder. It must contain a steamapps folder."
      : "Steam was not found automatically. Enter its folder, for example C:\\Program Files (x86)\\Steam.";
  }
}

startWithWindows.addEventListener("change", () => void save({ startWithWindows: startWithWindows.checked }));

maxResults.addEventListener("change", () => {
  const n = Math.round(Number(maxResults.value));
  if (!Number.isFinite(n) || n < 1 || n > 500) {
    maxResults.value = String(settings.maxResults);
    return;
  }
  void save({ maxResults: n });
});

steamPath.addEventListener("change", () => void save({ steamPath: steamPath.value.trim() }));
steamPath.addEventListener("keydown", (e) => {
  if (e.key === "Enter") steamPath.blur();
});

// ---------- Keyboard shortcut ----------

function startCapture(): void {
  capturing = true;
  void invoke("begin_hotkey_capture");
  hotkeyField.value = "";
  hotkeyField.placeholder = "Press the new shortcut";
  hotkeyField.classList.add("capturing");
  hotkeyChange.disabled = true;
  setHotkeyHint("Press the new combination. Escape cancels.", false);
  hotkeyField.focus();
}

function stopCapture(cancelled: boolean): void {
  if (!capturing) return;
  capturing = false;
  if (cancelled) void invoke("cancel_hotkey_capture");
  hotkeyField.placeholder = "";
  hotkeyField.classList.remove("capturing");
  hotkeyField.value = settings.hotkey;
  hotkeyChange.disabled = false;
  if (cancelled) setHotkeyHint(HOTKEY_HINT, false);
}

function setHotkeyHint(text: string, error: boolean): void {
  hotkeyHint.textContent = text;
  hotkeyHint.classList.toggle("error", error);
}

hotkeyChange.addEventListener("click", startCapture);
hotkeyField.addEventListener("blur", () => stopCapture(true));
hotkeyField.addEventListener("keydown", (e) => {
  if (!capturing) return;
  e.preventDefault();
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
  if (e.key === "Escape" && plain) {
    stopCapture(true);
    hotkeyChange.focus();
    return;
  }
  const hotkey = hotkeyFromEvent(e);
  if (!hotkey) return;
  hotkeyField.value = hotkey;
  if (needsModifier(e)) {
    setHotkeyHint("Use at least one modifier (Ctrl, Alt, Shift or Win), or a function key.", true);
    return;
  }
  invoke("set_hotkey", { hotkey })
    .then(() => {
      settings.hotkey = hotkey;
      stopCapture(false);
      setHotkeyHint(`Shortcut changed to ${hotkey}.`, false);
      hotkeyChange.focus();
    })
    .catch((error: unknown) => setHotkeyHint(String(error), true));
});

// ---------- Font ----------

const fontLabel = (family: string) => family || DEFAULT_FONT_LABEL;
const cssFont = (family: string) => (family ? `"${family}", var(--font-default)` : "var(--font-default)");

/** Families matching the typed text, or all of them while the field shows the current font. */
function visibleFamilies(): string[] {
  const text = familyInput.value.trim().toLowerCase();
  if (!text || familyInput.value === fontLabel(settings.fontFamily)) return families;
  return families.filter((f) => fontLabel(f).toLowerCase().includes(text));
}

function renderFamilies(): void {
  const items = visibleFamilies().map((family) => {
    const li = document.createElement("li");
    li.role = "option";
    li.id = `family-${families.indexOf(family)}`;
    li.dataset.family = family;
    li.textContent = fontLabel(family);
    li.style.fontFamily = cssFont(family);
    if (family === settings.fontFamily) li.setAttribute("aria-selected", "true");
    return li;
  });
  familyList.replaceChildren(...items);
  const selected = familyList.querySelector<HTMLElement>('[aria-selected="true"]');
  if (selected) familyInput.setAttribute("aria-activedescendant", selected.id);
  else familyInput.removeAttribute("aria-activedescendant");
}

function renderSizes(): void {
  const items = SIZES.map((size) => {
    const li = document.createElement("li");
    li.role = "option";
    li.id = `size-${size}`;
    li.dataset.size = String(size);
    li.textContent = String(size);
    if (size === settings.fontSize) li.setAttribute("aria-selected", "true");
    return li;
  });
  sizeList.replaceChildren(...items);
}

function renderPreview(): void {
  const names = settings.sampleGames.length > 0 ? settings.sampleGames : FALLBACK_SAMPLES;
  preview.style.fontFamily = cssFont(settings.fontFamily);
  preview.style.fontSize = `${settings.fontSize}px`;
  preview.replaceChildren(
    ...names.map((name, i) => {
      const li = document.createElement("li");
      li.textContent = name;
      if (i === 0) li.classList.add("selected");
      return li;
    }),
  );
}

function scrollSelectedIntoView(): void {
  for (const list of [familyList, sizeList]) {
    list.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  }
}

function showFont(): void {
  familyInput.value = fontLabel(settings.fontFamily);
  sizeInput.value = String(settings.fontSize);
  renderFamilies();
  renderSizes();
  renderPreview();
  scrollSelectedIntoView();
}

/** Shows the new font at once and saves it shortly after, so arrowing through the list stays quick. */
function setFont(fontFamily: string, fontSize: number): void {
  settings.fontFamily = fontFamily;
  settings.fontSize = fontSize;
  showFont();
  window.clearTimeout(fontSaveTimer);
  fontSaveTimer = window.setTimeout(() => void save({ fontFamily, fontSize }), 250);
}

/** Moves the selection within the visible families (arrow keys in the font field). */
function stepFamily(step: number): void {
  const visible = visibleFamilies();
  if (visible.length === 0) return;
  const index = visible.indexOf(settings.fontFamily);
  const next = index < 0 ? 0 : Math.min(Math.max(index + step, 0), visible.length - 1);
  setFont(visible[next], settings.fontSize);
}

familyInput.addEventListener("input", renderFamilies);
familyInput.addEventListener("keydown", (e) => {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    stepFamily(e.key === "ArrowDown" ? 1 : -1);
  } else if (e.key === "Enter") {
    e.preventDefault();
    const visible = visibleFamilies();
    const exact = visible.find((f) => fontLabel(f).toLowerCase() === familyInput.value.trim().toLowerCase());
    const choice = exact ?? visible[0];
    if (choice !== undefined) setFont(choice, settings.fontSize);
    familyInput.select();
  } else if (e.key === "Escape") {
    e.preventDefault();
    showFont();
  }
});
familyInput.addEventListener("blur", () => {
  const typed = familyInput.value.trim().toLowerCase();
  const exact = families.find((f) => fontLabel(f).toLowerCase() === typed);
  if (exact !== undefined && exact !== settings.fontFamily) setFont(exact, settings.fontSize);
  else showFont();
});

familyList.addEventListener("mousedown", (e) => e.preventDefault()); // keep focus in the field
familyList.addEventListener("click", (e) => {
  const item = (e.target as HTMLElement).closest<HTMLElement>("li");
  if (item?.dataset.family !== undefined) setFont(item.dataset.family, settings.fontSize);
});

function applySizeField(): void {
  const n = Math.round(Number(sizeInput.value));
  if (Number.isFinite(n) && n >= 8 && n <= 40) setFont(settings.fontFamily, n);
  else sizeInput.value = String(settings.fontSize);
}

sizeInput.addEventListener("change", applySizeField);
sizeInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    e.preventDefault();
    applySizeField();
  }
});
sizeList.addEventListener("mousedown", (e) => e.preventDefault());
sizeList.addEventListener("click", (e) => {
  const item = (e.target as HTMLElement).closest<HTMLElement>("li");
  if (item?.dataset.size) setFont(settings.fontFamily, Number(item.dataset.size));
});

fontReset.addEventListener("click", () => setFont("", DEFAULT_FONT_SIZE));

// ---------- Startup ----------

async function init(): Promise<void> {
  settings = await invoke<Settings>("get_settings");
  showGeneral();
  showFont();
  families = ["", ...(await invoke<string[]>("font_families"))];
  // A configured font that is not installed stays listed, so the field still shows it.
  if (settings.fontFamily && !families.includes(settings.fontFamily)) families.push(settings.fontFamily);
  showFont();
}

void init();
