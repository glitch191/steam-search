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

interface Appearance {
  fontFamily: string;
  fontSize: number;
}

const MESSAGES: Partial<Record<Status, string>> = {
  "no-steam": "Steam installation not found. Set the Steam folder in Settings (tray icon menu).",
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
let lastWidth = 0;
let lastHeight = 0;

/** Narrowest window, in CSS pixels; it widens to show longer names in full. */
const MIN_WIDTH = 400;

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
  fitWindow();
}

/** Tooltips for names still cut off, which only happens when the window is at its widest. */
function updateTooltips(): void {
  for (const li of list.children as HTMLCollectionOf<HTMLElement>) {
    const span = li.firstElementChild as HTMLElement;
    if (span.scrollWidth > span.clientWidth) li.title = li.textContent ?? "";
    else li.removeAttribute("title");
  }
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

/** Horizontal padding and borders of an element, in CSS pixels. */
function horizontalChrome(el: Element): number {
  const s = getComputedStyle(el);
  return [s.paddingLeft, s.paddingRight, s.borderLeftWidth, s.borderRightWidth].reduce((sum, v) => sum + parseFloat(v), 0);
}

const measureContext = document.createElement("canvas").getContext("2d");

/**
 * Width that shows the search field placeholder and every listed name in full, never below
 * MIN_WIDTH. Independent of the current window width, so resizing does not feed back into it.
 */
function contentWidth(): number {
  let widest = 0;
  if (measureContext) {
    measureContext.font = getComputedStyle(input).font;
    widest = measureContext.measureText(input.placeholder).width + horizontalChrome(input);
  }
  const first = list.firstElementChild;
  if (first) {
    const scrollbar = list.offsetWidth - list.clientWidth;
    const rowChrome = horizontalChrome(first) + scrollbar;
    for (const li of list.children) {
      widest = Math.max(widest, (li.firstElementChild as HTMLElement).scrollWidth + rowChrome);
    }
  }
  return Math.max(MIN_WIDTH, Math.ceil(widest + horizontalChrome(app)));
}

/** Asks for a window that fits the content. The window may cap the width at 90 % of the screen. */
function fitWindow(): void {
  const width = contentWidth();
  const height = Math.ceil(app.getBoundingClientRect().height);
  if (width !== lastWidth || height !== lastHeight) {
    lastWidth = width;
    lastHeight = height;
    void invoke("fit", { width, height });
  }
}

// A width change can rewrap the status message (new height) and cut off or reveal names.
new ResizeObserver(() => {
  updateTooltips();
  fitWindow();
}).observe(app);

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

/** Applies the game list font chosen in Settings. An unknown font falls back to the default one. */
async function applyAppearance(): Promise<void> {
  const { fontFamily, fontSize } = await invoke<Appearance>("appearance");
  const root = document.documentElement.style;
  if (fontFamily) root.setProperty("--results-font", `"${fontFamily}", var(--font-default)`);
  else root.removeProperty("--results-font");
  root.setProperty("--results-font-size", `${fontSize}px`);
  if (hits.length > 0) render("ok");
  else fitWindow();
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
void listen("appearance-changed", () => void applyAppearance());

void applyAppearance().then(runSearch);
