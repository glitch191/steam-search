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

/** Runs a search for the current field value. Stale responses are ignored. */
async function runSearch(): Promise<void> {
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
  input.value = "";
  requestId++;
  hits = [];
  selected = 0;
  void runSearch();
}

input.addEventListener("input", () => void runSearch());

input.addEventListener("keydown", (e) => {
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
void listen("hidden", clear);
void listen("library-updated", () => void runSearch());

void runSearch();
