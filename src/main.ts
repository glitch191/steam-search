import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./style.css";

interface Hit {
  appid: number;
  name: string;
  indices: number[];
}

type Status = "loading" | "no-steam" | "no-games" | "no-match" | "ok";

interface SearchResult {
  status: Status;
  hits: Hit[];
}

const MESSAGES: Partial<Record<Status, string>> = {
  "no-steam": "Steam installation not found. Set steamPath in config.json.",
  "no-games": "No installed games found",
  "no-match": "No installed game matches",
};

const app = document.getElementById("app") as HTMLElement;
const input = document.getElementById("query") as HTMLInputElement;
const list = document.getElementById("results") as HTMLUListElement;
const statusLine = document.getElementById("status") as HTMLParagraphElement;

let hits: Hit[] = [];
let selected = 0;
let requestId = 0;
let lastHeight = 0;
let capturing = false;

const SEARCH_PLACEHOLDER = input.placeholder;
const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS"]);

/** Runs a search for the current field value. Stale responses are ignored. */
async function runSearch(): Promise<void> {
  if (capturing) return;
  const id = ++requestId;
  const result = await invoke<SearchResult>("search", { query: input.value });
  if (id !== requestId) return;
  hits = result.hits;
  selected = 0;
  render(result.status);
}

/** Bold spans for the matched characters. Indices are Unicode code point positions. */
function nameWithHighlights(hit: Hit): DocumentFragment {
  const fragment = document.createDocumentFragment();
  const marked = new Set(hit.indices);
  let run = "";
  let runBold = false;
  const flush = () => {
    if (!run) return;
    if (runBold) {
      const b = document.createElement("b");
      b.textContent = run;
      fragment.append(b);
    } else {
      fragment.append(run);
    }
    run = "";
  };
  Array.from(hit.name).forEach((ch, i) => {
    const bold = marked.has(i);
    if (bold !== runBold) {
      flush();
      runBold = bold;
    }
    run += ch;
  });
  flush();
  return fragment;
}

function render(status: Status): void {
  const rows = hits.map((hit, i) => {
    const li = document.createElement("li");
    li.id = `result-${i}`;
    li.role = "option";
    li.dataset.index = String(i);
    const span = document.createElement("span");
    span.className = "name";
    span.append(nameWithHighlights(hit));
    li.append(span);
    return li;
  });
  list.replaceChildren(...rows);
  list.hidden = rows.length === 0;

  const showMessage = status !== "ok" && status !== "loading" && (status !== "no-match" || input.value.trim() !== "");
  statusLine.textContent = showMessage ? (MESSAGES[status] ?? "") : "";
  statusLine.hidden = !showMessage;

  updateSelection(-1);
  // Layout reads after all DOM writes: tooltips for truncated names, then window height.
  for (const li of rows) {
    const span = li.firstElementChild as HTMLElement;
    if (span.scrollWidth > span.clientWidth) li.title = li.textContent ?? "";
  }
  fitWindow();
}

function updateSelection(previous: number): void {
  if (previous >= 0) list.children[previous]?.removeAttribute("aria-selected");
  const row = list.children[selected] as HTMLElement | undefined;
  if (row) {
    row.setAttribute("aria-selected", "true");
    row.scrollIntoView({ block: "nearest" });
    input.setAttribute("aria-activedescendant", row.id);
  } else {
    input.removeAttribute("aria-activedescendant");
  }
}

function fitWindow(): void {
  const height = Math.ceil(app.getBoundingClientRect().height);
  if (height !== lastHeight) {
    lastHeight = height;
    void invoke("fit", { height });
  }
}

function launch(index: number): void {
  const hit = hits[index];
  if (!hit || !Number.isInteger(hit.appid)) return;
  void invoke("launch", { appid: hit.appid });
}

function clear(): void {
  capturing = false;
  input.readOnly = false;
  input.placeholder = SEARCH_PLACEHOLDER;
  input.value = "";
  requestId++;
  hits = [];
  selected = 0;
  void runSearch();
}

input.addEventListener("input", () => void runSearch());

input.addEventListener("keydown", (e) => {
  if (capturing) {
    captureKey(e);
    return;
  }
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    if (hits.length === 0) return;
    const previous = selected;
    const step = e.key === "ArrowDown" ? 1 : -1;
    selected = (selected + step + hits.length) % hits.length;
    updateSelection(previous);
  } else if (e.key === "Enter") {
    e.preventDefault();
    launch(selected);
  } else if (e.key === "Escape") {
    e.preventDefault();
    void invoke("hide");
  }
});

list.addEventListener("mousedown", (e) => e.preventDefault()); // keep focus in the field
list.addEventListener("click", (e) => {
  const row = (e.target as HTMLElement).closest("li");
  if (row?.dataset.index) launch(Number(row.dataset.index));
});

void listen("shown", () => {
  input.focus();
  void invoke("ready");
});
void listen<string>("capture-hotkey", (e) => enterCapture(e.payload));
void listen("hidden", clear);
void listen("library-updated", () => void runSearch());

void runSearch();

/** Shows a single message line under the field, without results. */
function showMessage(text: string): void {
  hits = [];
  list.replaceChildren();
  list.hidden = true;
  statusLine.textContent = text;
  statusLine.hidden = false;
  fitWindow();
}

function enterCapture(current: string): void {
  capturing = true;
  requestId++;
  input.value = "";
  input.readOnly = true;
  input.placeholder = "Press the new shortcut";
  input.focus();
  showMessage(`Current shortcut: ${current}. Press Escape to cancel.`);
}

/** Builds a hotkey string such as "Ctrl+Shift+Insert" from a key press, or null while only modifiers are held. */
function hotkeyFromEvent(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Win");
  const key = e.code.replace(/^Key(?=[A-Z]$)/, "").replace(/^Digit(?=\d$)/, "");
  parts.push(key);
  return parts.join("+");
}

function captureKey(e: KeyboardEvent): void {
  e.preventDefault();
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
  if (e.key === "Escape" && plain) {
    void invoke("hide");
    return;
  }
  const hotkey = hotkeyFromEvent(e);
  if (!hotkey) return;
  input.value = hotkey;
  if (plain && !/^F\d+$/.test(e.code)) {
    showMessage("Use at least one modifier (Ctrl, Alt, Shift or Win), or a function key.");
    return;
  }
  invoke("set_hotkey", { hotkey }).catch((error: unknown) => showMessage(String(error)));
}
