// Toolbar popup: is the app running and paired, is this tab being counted,
// today's web time, the rules' progress and the tray menu's controls.
// Built from GET /state and GET /stats/web as the app serves them.

import { findPort, get, post, NotPaired } from "./api";
import { browserName, ext, settings } from "./browser";
import {
  ago,
  badgeText,
  canAct,
  COUNTING_TEXT,
  counting,
  duration,
  hostOf,
  ruleFraction,
  ruleProgress,
  type AppState,
  type HostUsage,
  type RuleSnapshot,
} from "./format";

const BROWSER = browserName();
const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: { className?: string; text?: string } = {},
  children: Node[] = [],
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (props.className) e.className = props.className;
  if (props.text !== undefined) e.textContent = props.text;
  for (const c of children) e.append(c);
  return e;
}

function setStatus(text: string, cls: string, link?: string) {
  const s = $("status");
  s.className = cls;
  s.replaceChildren(text);
  if (link) {
    const a = el("a", { text: link });
    a.href = "#";
    a.addEventListener("click", openOptions);
    s.append(" ", a);
  }
}

function openOptions(e: Event) {
  e.preventDefault();
  void ext.runtime.openOptionsPage();
  window.close();
}

async function renderNow(state: AppState) {
  const { last } = await ext.storage.session.get("last");
  const l = last as { url: string | null; title: string | null } | undefined;
  const c = counting(state, BROWSER);
  const pill = $("now-state");
  pill.textContent = COUNTING_TEXT[c];
  pill.className = `pill ${c === "counting" ? "ok" : c === "paused" ? "warn" : ""}`;
  const host = l?.url?.startsWith(location.origin) ? "Nagsworth extension page" : hostOf(l?.url);
  $("now-site").textContent = host ?? (l ? "A private or unreadable tab" : "No tab reported yet");
  $("now-title").textContent = l?.title ?? "";
}

function renderToday(hosts: HostUsage[]) {
  const total = hosts.reduce((n, h) => n + h.ms, 0);
  $("today-total").textContent = duration(total);
  const list = $("hosts");
  if (hosts.length === 0) {
    list.replaceChildren(el("p", { className: "muted", text: "No web time yet today." }));
    return;
  }
  list.replaceChildren(
    ...hosts.slice(0, 5).map((h) =>
      el("div", { className: "row" }, [
        el("span", { className: "name", text: h.host }),
        el("span", { className: "val", text: duration(h.ms) }),
      ]),
    ),
  );
}

function renderRules(rules: RuleSnapshot[], now: number) {
  const enabled = rules.filter((r) => r.enabled);
  const box = $("rules");
  if (enabled.length === 0) {
    box.replaceChildren(el("p", { className: "muted", text: "No rules are on." }));
    return;
  }
  box.replaceChildren(
    ...enabled.map((r) => {
      const frac = ruleFraction(r);
      const fill = el("div", { className: frac >= 1 ? "over" : frac >= 0.8 ? "warn" : "" });
      fill.style.width = `${Math.round(frac * 100)}%`;
      const st = r.state?.state;
      const nagging = st === "active" || st === "pending";
      const item = el("div", { className: "rule" }, [
        el("div", { className: "row" }, [
          el("span", { className: "name", text: r.name }),
          el("span", { className: "val", text: ruleProgress(r, now) }),
        ]),
      ]);
      if ((r.target_ms ?? 0) > 0) item.append(el("div", { className: "bar" }, [fill]));
      if (nagging && r.message) item.append(el("p", { className: "msg", text: r.message }));
      const act = canAct(r);
      if (act.snooze || act.skip) {
        const acts = el("div", { className: "acts" });
        if (act.snooze) {
          acts.append(control(`Snooze (${r.snoozes_left} left)`, "/control/snooze", { rule_id: r.id }));
        }
        if (act.skip) acts.append(control("Skip", "/control/skip", { rule_id: r.id }));
        item.append(acts);
      }
      return item;
    }),
  );
}

function control(label: string, path: string, body: unknown): HTMLButtonElement {
  const b = el("button", { text: label });
  b.addEventListener("click", () => void run(b, path, body));
  return b;
}

async function run(button: HTMLButtonElement, path: string, body: unknown) {
  button.disabled = true;
  const msg = $("control-msg");
  try {
    await post(path, body);
    msg.textContent = "";
  } catch (e) {
    msg.textContent = e instanceof Error ? e.message : String(e);
    msg.className = "err";
  }
  await render();
}

async function render() {
  const s = await settings();
  const port = await findPort();
  $("paired").hidden = true;
  if (port === null) {
    setStatus("The Nagsworth app is not running on this computer.", "err");
    return;
  }
  if (!s.token) {
    setStatus(`App found on port ${port}. Not paired yet.`, "warn", "Pair in Settings");
    return;
  }
  let state: AppState;
  let hosts: HostUsage[];
  try {
    [state, { hosts = [] }] = await Promise.all([
      get<AppState>("/state"),
      get<{ hosts?: HostUsage[] }>("/stats/web"),
    ]);
  } catch (e) {
    if (e instanceof NotPaired) setStatus("The app no longer accepts this browser.", "err", "Pair again");
    else setStatus(e instanceof Error ? e.message : String(e), "err");
    return;
  }
  const { lastSent } = await ext.storage.local.get("lastSent");
  const now = state.now ?? Date.now();
  const sent = typeof lastSent === "number" ? `, last sent ${ago(lastSent, Date.now())}` : "";
  setStatus(`Paired as ${BROWSER} on port ${port}${sent}.`, "ok");
  $("paired").hidden = false;

  await renderNow(state);
  renderToday(hosts);
  renderRules(state.engine?.rules ?? [], now);
  $("pause").hidden = Boolean(state.tracking_paused);
  $("resume").hidden = !state.tracking_paused;

  const { badge } = await ext.storage.local.get("badge");
  if (badge === true) {
    await ext.action.setBadgeText({ text: badgeText(hosts.reduce((n, h) => n + h.ms, 0)) });
  }
}

$("open-options").addEventListener("click", openOptions);
$("break").addEventListener("click", (e) => void run(e.target as HTMLButtonElement, "/control/break/start", {}));
$("pause").addEventListener("click", (e) =>
  void run(e.target as HTMLButtonElement, "/control/pause", { minutes: 30 }),
);
$("resume").addEventListener("click", (e) => void run(e.target as HTMLButtonElement, "/control/resume", {}));

void render();
