// Pilote CDP de la WebView2 de Cadence, pour les tests en conditions réelles.
// Aucune dépendance (Node ≥ 22 : fetch et WebSocket natifs).
//
// Cadres : « app » (la page de Cadence) et « plugin <id> » (iframe d'un plugin, hors processus :
// cible CDP distincte, attachée par Target.setAutoAttach en sessions aplaties). Toutes les
// commandes voient les deux ; les coordonnées sont celles de la fenêtre.
//
//   voir [--tout] [--cadre app|plugin|<id>]
//        éléments cliquables à l'écran, numérotés (#n). Les éléments hors écran ou recouverts
//        (fenêtre modale…) sont comptés ; --tout les liste avec [hors écran] / [masqué].
//   texte [max] [--cadre …] [--delai s]      texte visible par cadre (délai par cadre, défaut 5 s)
//   clic <#n|texte> [rang] [--cadre …]       fait défiler l'élément dans la vue puis vrai clic souris.
//        Texte : libellé exact d'abord, puis début, puis inclusion ; à égalité, le plugin affiché
//        avant l'app. rang = n-ième correspondance (0 = première).
//   saisir <#n|texte> <valeur> [--cadre …]   champ texte / contenteditable (remplace le contenu) ;
//        <select> : choisit l'option par libellé (ou valeur).
//   cocher <#n|texte> [oui|non] [--cadre …]  case à cocher, radio, interrupteur (défaut : oui)
//   taper <texte>                            insère du texte là où est le focus
//   touche <Enter|Escape|Tab|ctrl+a|…> [n]   envoie une touche (n fois), modificateurs ctrl/alt/shift/meta
//   defiler <#n|texte|haut|bas|+px|-px> [--cadre …]
//        amène un élément au centre, ou fait défiler la zone principale du cadre actif
//   attendre <texte> [secondes] [--absent] [--cadre …]  attend qu'un texte apparaisse (ou parte)
//   capture <fichier.png> [--cadre #n|texte|x,y,l,h]... [--note "texte"]
//        capture de la fenêtre ; chaque --cadre entoure une zone en rouge ; --note pose une
//        étiquette sous le premier cadre, ou au-dessus / à côté s'il n'y a pas la place.
//   eval <expression> [--cadre …]            évalue dans un cadre (diagnostic seulement)
//   console [--effacer]                      erreurs et avertissements depuis le chargement de
//        chaque cadre (relecture CDP + collecteur injecté), et ceux de l'écoute continue
//   console --ecouter [secondes]             écoute continue (défaut 3600 s) vers le fichier
//        tampon ; à lancer en arrière-plan
//   taille [<largeur> <hauteur> | max]       redimensionne la vraie fenêtre (Win32, durable) pour
//        que la zone de rendu fasse largeur × hauteur px CSS ; sans argument : taille actuelle
//
// Variables : CDP_PORT (défaut 9339), CDP_ETAT (dossier d'état, défaut %TEMP%\cadence-qa-pilote :
// numérotation de « voir », tampon console). Relancer « voir » après chaque changement d'écran ;
// un #n périmé est retrouvé par son libellé quand c'est sans ambiguïté.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const PORT = process.env.CDP_PORT ?? "9339";
const ETAT = process.env.CDP_ETAT ?? path.join(os.tmpdir(), "cadence-qa-pilote");
fs.mkdirSync(ETAT, { recursive: true });
const STATE = path.join(ETAT, "elements.json");
const CONSOLE_FILE = path.join(ETAT, "console.log");
const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");

// ── Arguments ────────────────────────────────────────────────────────────────────────────────
const [cmd, ...rawArgs] = process.argv.slice(2);
const pos = [];
const opt = { cadres: [] };
for (let i = 0; i < rawArgs.length; i++) {
  const a = rawArgs[i];
  if (a === "--cadre") opt.cadres.push(rawArgs[++i]);
  else if (a === "--note") opt.note = rawArgs[++i];
  else if (a === "--delai") opt.delai = Number(rawArgs[++i]);
  else if (a === "--tout") opt.tout = true;
  else if (a === "--absent") opt.absent = true;
  else if (a === "--effacer") opt.effacer = true;
  else if (a === "--ecouter") opt.ecouter = true;
  else pos.push(a);
}
// Hors capture, --cadre choisit le cadre : app, plugin (le plugin affiché) ou l'id d'un plugin.
const frameFilter = cmd === "capture" ? undefined : opt.cadres[0];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const norm = (s) => (s ?? "").replace(/\s+/g, " ").trim().toLowerCase();

// Garde-fou : aucune commande ne reste bloquée (sauf attendre et l'écoute console).
const longCmd = cmd === "attendre" || (cmd === "console" && opt.ecouter);
if (!longCmd)
  setTimeout(() => {
    console.log(`Erreur : « ${cmd} » sans réponse après 90 s (page figée ?).`);
    process.exit(1);
  }, 90_000).unref();

// ── Connexion CDP ────────────────────────────────────────────────────────────────────────────
const targets = await fetch(`http://127.0.0.1:${PORT}/json/list`)
  .then((r) => r.json())
  .catch(() => {
    console.error(`Pas de WebView2 en débogage sur le port ${PORT} : lancez d'abord scripts/lancer.ps1.`);
    process.exit(2);
  });
const pages = targets.filter((t) => t.type === "page" && !t.url.startsWith("devtools"));
const page = pages.find((t) => t.url.includes(":1439")) ?? pages[0];
if (!page) {
  console.error("Aucune page Cadence trouvée.");
  process.exit(2);
}

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, ko) => {
  ws.onopen = ok;
  ws.onerror = () => ko(new Error("connexion CDP refusée"));
});

const ROOT = "";
let seq = 0;
const pending = new Map();
const sessions = new Map([[ROOT, { id: ROOT, targetId: page.id, parent: null, url: page.url }]]);
const contexts = new Map(); // `${session}|${frameId}` → id du contexte principal
const consoleLines = [];
const listeners = [];

function send(method, params = {}, session = ROOT, timeout = 15_000) {
  return new Promise((ok, ko) => {
    const id = ++seq;
    const timer = setTimeout(() => {
      pending.delete(id);
      ko(new Error(`${method} : pas de réponse en ${timeout / 1000} s`));
    }, timeout);
    pending.set(id, (msg) => {
      clearTimeout(timer);
      if (msg.error) ko(new Error(`${method} : ${msg.error.message}`));
      else ok(msg.result);
    });
    ws.send(JSON.stringify({ id, method, params, ...(session ? { sessionId: session } : {}) }));
  });
}

function fmtConsole(sessionId, kind, text) {
  const s = sessions.get(sessionId ?? ROOT);
  const where = s && s.id !== ROOT ? frameName(s.url) : "app";
  const t = new Date().toTimeString().slice(0, 8);
  return `${t} ${where} [${kind}] ${text}`;
}

ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
    return;
  }
  const s = msg.sessionId ?? ROOT;
  const p = msg.params ?? {};
  switch (msg.method) {
    case "Target.attachedToTarget":
      sessions.set(p.sessionId, { id: p.sessionId, targetId: p.targetInfo.targetId, parent: s, url: p.targetInfo.url, type: p.targetInfo.type });
      prepareSession(p.sessionId, p.waitingForDebugger).catch(() => {});
      break;
    case "Target.detachedFromTarget":
      sessions.delete(p.sessionId);
      break;
    case "Target.targetInfoChanged":
      for (const x of sessions.values()) if (x.targetId === p.targetInfo.targetId) x.url = p.targetInfo.url;
      break;
    case "Runtime.executionContextCreated":
      if (p.context.auxData?.isDefault) contexts.set(`${s}|${p.context.auxData.frameId}`, p.context.id);
      break;
    case "Runtime.executionContextDestroyed":
      for (const [k, v] of contexts) if (v === p.executionContextId && k.startsWith(s + "|")) contexts.delete(k);
      break;
    case "Runtime.executionContextsCleared":
      for (const k of [...contexts.keys()]) if (k.startsWith(s + "|")) contexts.delete(k);
      break;
    case "Runtime.consoleAPICalled":
      if (["error", "warning", "assert"].includes(p.type))
        consoleLines.push(fmtConsole(s, p.type, p.args.map((a) => a.value ?? a.description ?? a.unserializableValue ?? "").join(" ")));
      break;
    case "Runtime.exceptionThrown":
      consoleLines.push(fmtConsole(s, "exception", p.exceptionDetails.exception?.description ?? p.exceptionDetails.text));
      break;
    case "Log.entryAdded":
      if (["error", "warning"].includes(p.entry.level)) consoleLines.push(fmtConsole(s, p.entry.source, p.entry.text + (p.entry.url ? ` (${p.entry.url})` : "")));
      break;
  }
  for (const l of listeners) l(msg);
};

async function prepareSession(sessionId, waiting) {
  await send("Runtime.enable", {}, sessionId);
  if (cmd === "console") await send("Log.enable", {}, sessionId).catch(() => {});
  // Iframes imbriquées dans le plugin, s'il y en a.
  await send("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: false, flatten: true }, sessionId).catch(() => {});
  if (waiting) await send("Runtime.runIfWaitingForDebugger", {}, sessionId).catch(() => {});
}

await prepareSession(ROOT);
await sleep(150); // laisse arriver les attachements d'iframes et leurs contextes
for (let i = 0; i < 10 && [...sessions.values()].some((s) => s.id !== ROOT && ![...contexts.keys()].some((k) => k.startsWith(s.id + "|"))); i++) await sleep(100);

// ── Cadres ───────────────────────────────────────────────────────────────────────────────────
function frameName(url) {
  const m = url.match(/cadence-plugin(?:\.localhost|:\/\/localhost)\/([^/]+)/) ?? url.match(/mock-plugins\/([^/]+)/);
  return "plugin " + (m?.[1] ?? url.replace(/^https?:\/\//, "").slice(0, 40));
}

async function evalIn(frame, expression, timeout = 15_000) {
  const res = await send("Runtime.evaluate", { expression, contextId: frame.ctx, awaitPromise: true, returnByValue: true, userGesture: true }, frame.session, timeout);
  if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description ?? res.exceptionDetails.text);
  return res.result?.value;
}

// Rectangle de l'iframe hôte dans le document parent (null si l'iframe est cachée).
async function ownerBox(ownerSession, frameId) {
  const owner = await send("DOM.getFrameOwner", { frameId }, ownerSession).catch(() => null);
  if (!owner?.backendNodeId) return null;
  const obj = await send("DOM.resolveNode", { backendNodeId: owner.backendNodeId }, ownerSession).catch(() => null);
  if (!obj?.object?.objectId) return null;
  const r = await send(
    "Runtime.callFunctionOn",
    {
      objectId: obj.object.objectId,
      returnByValue: true,
      functionDeclaration: `function () {
        const r = this.getBoundingClientRect(), st = getComputedStyle(this);
        const pl = parseFloat(st.paddingLeft) || 0, pt = parseFloat(st.paddingTop) || 0;
        return { x: r.left + this.clientLeft + pl, y: r.top + this.clientTop + pt, w: this.clientWidth - pl - (parseFloat(st.paddingRight) || 0),
          h: this.clientHeight - pt - (parseFloat(st.paddingBottom) || 0), title: this.title || this.name || '',
          visible: st.visibility !== 'hidden' && st.display !== 'none' && r.width > 1 && r.height > 1 };
      }`,
    },
    ownerSession,
  ).catch(() => null);
  const box = r?.result?.value;
  return box ? { ...box, objectId: obj.object.objectId } : null;
}

let rootSize;
async function frames() {
  const out = [];
  const oopIds = new Set([...sessions.values()].filter((s) => s.id !== ROOT).map((s) => s.targetId));
  // Sessions dans l'ordre : la page d'abord, puis les iframes (parent avant enfant).
  const order = [];
  const visit = (sid) => {
    order.push(sessions.get(sid));
    for (const s of sessions.values()) if (s.parent === sid && s.id !== sid) visit(s.id);
  };
  visit(ROOT);
  const byKey = new Map();
  for (const s of order) {
    const tree = await send("Page.getFrameTree", {}, s.id).catch(() => null);
    if (!tree) continue;
    const walk = async (node, parentFrame) => {
      const f = node.frame;
      if (oopIds.has(f.id) && s.id === ROOT && parentFrame) return; // traité par sa propre session
      const isSessionRoot = !parentFrame;
      const fr = { key: `${s.targetId}|${f.id}`, session: s.id, frameId: f.id, url: f.url, ctx: contexts.get(`${s.id}|${f.id}`) };
      if (s.id === ROOT && isSessionRoot) {
        fr.where = "app";
        fr.off = { x: 0, y: 0 };
        fr.visible = true;
      } else {
        // Iframe : même processus (propriétaire dans la même session) ou hors processus (session parente).
        const ownerSession = isSessionRoot ? s.parent : s.id;
        const parentKey = isSessionRoot ? [...byKey.values()].find((x) => x.session === s.parent && x.isSessionRoot)?.key : parentFrame.key;
        const parent = byKey.get(parentKey);
        const box = await ownerBox(ownerSession, f.id);
        fr.where = frameName(f.url);
        fr.title = box?.title;
        fr.visible = !!box?.visible && (parent?.visible ?? true);
        fr.off = { x: (parent?.off.x ?? 0) + (box?.x ?? 0), y: (parent?.off.y ?? 0) + (box?.y ?? 0) };
        fr.size = box ? { w: box.w, h: box.h } : null;
        fr.owner = box ? { session: ownerSession, objectId: box.objectId } : null;
        fr.parentKey = parentKey;
      }
      fr.isSessionRoot = isSessionRoot;
      byKey.set(fr.key, fr);
      if (fr.ctx) out.push(fr);
      for (const c of node.childFrames ?? []) await walk(c, fr);
    };
    await walk(tree.frameTree, null);
  }
  const app = out.find((f) => f.where === "app");
  rootSize = app ? await evalIn(app, "[innerWidth, innerHeight, devicePixelRatio]").catch(() => null) : null;
  // Zone visible de chaque cadre, dans ses propres coordonnées.
  for (const f of out) {
    const [W, H] = rootSize ?? [1e5, 1e5];
    let x0 = -f.off.x, y0 = -f.off.y, x1 = W - f.off.x, y1 = H - f.off.y;
    if (f.size) [x0, y0, x1, y1] = [Math.max(x0, 0), Math.max(y0, 0), Math.min(x1, f.size.w), Math.min(y1, f.size.h)];
    f.clip = { x0, y0, x1, y1 };
    if (x1 - x0 < 2 || y1 - y0 < 2) f.visible = false;
  }
  return out;
}

// Filtre --cadre : app, plugin (les plugins affichés) ou l'id d'un plugin.
function pickFrames(all, filter) {
  const vis = all.filter((f) => f.visible);
  if (!filter) return vis;
  const w = filter.toLowerCase();
  const r = w === "app" ? vis.filter((f) => f.where === "app") : w === "plugin" ? vis.filter((f) => f.where !== "app") : vis.filter((f) => f.where.toLowerCase().includes(w));
  if (!r.length) throw new Error(`Aucun cadre affiché « ${filter} » (cadres : ${vis.map((f) => f.where).join(", ")}).`);
  return r;
}

// ── Collecte des éléments ───────────────────────────────────────────────────────────────────
const SEL = "button,a[href],[role=button],[role=menuitem],[role=menuitemradio],[role=menuitemcheckbox],[role=tab],[role=checkbox],[role=radio],[role=option],[role=switch],[role=link],[role=combobox],[role=treeitem],input:not([type=hidden]),textarea,select,[contenteditable=''],[contenteditable=true],summary,[tabindex]:not([tabindex='-1'])";

// Bibliothèque injectée dans chaque cadre (monde principal) : registre des éléments + mesures.
const LIB = `(() => {
  if (window.__pilote?.v === 4) return;
  const P = window.__pilote = { v: 4, els: [] };
  const SEL = ${JSON.stringify(SEL)};
  P.norm = (s) => (s || '').replace(/\\s+/g, ' ').trim();
  P.label = (el) => {
    const by = el.getAttribute('aria-labelledby');
    const t = el.getAttribute('aria-label') || (by && by.split(' ').map((i) => document.getElementById(i)?.innerText || '').join(' ')) ||
      (el.labels && el.labels[0] && el.labels[0].innerText) || (el.tagName === 'SELECT' ? '' : el.innerText) || el.placeholder || el.title ||
      el.getAttribute('alt') || (/^(radio|checkbox|text|search|email|number|password|url|tel)$/.test(el.type) || el.tagName !== 'INPUT' ? '' : el.value) || '';
    return P.norm(t).slice(0, 90);
  };
  // Zone cliquable : une case native masquée (sr-only) se clique par son libellé.
  P.box = (el) => {
    let r = el.getBoundingClientRect();
    if ((r.width < 2 || r.height < 2) && (el.type === 'checkbox' || el.type === 'radio')) {
      const l = (el.labels && el.labels[0]) || el.closest('label');
      if (l) r = l.getBoundingClientRect();
    }
    return r;
  };
  P.hits = (el, h) => !!h && (h === el || el.contains(h) || ((h.closest('label') || {}).control === el) || (el.labels && [...el.labels].some((l) => l.contains(h))));
  // État d'un élément dans la zone visible clip : ok (point cliquable px,py), partiel, masqué, hors écran.
  P.measure = (el, clip) => {
    const r = P.box(el);
    const st = getComputedStyle(el);
    if (r.width < 2 || r.height < 2 || st.visibility === 'hidden' || st.display === 'none') return null;
    const ix0 = Math.max(r.left, clip.x0, 0), iy0 = Math.max(r.top, clip.y0, 0);
    const ix1 = Math.min(r.right, clip.x1, innerWidth), iy1 = Math.min(r.bottom, clip.y1, innerHeight);
    const m = { x: r.left, y: r.top, w: r.width, h: r.height };
    if (ix1 - ix0 < 2 || iy1 - iy0 < 2) return { ...m, state: 'hors écran' };
    const full = ix0 <= r.left + 1 && iy0 <= r.top + 1 && ix1 >= r.right - 1 && iy1 >= r.bottom - 1;
    const cands = [[(ix0 + ix1) / 2, (iy0 + iy1) / 2], [ix0 + 3, (iy0 + iy1) / 2], [(ix0 + ix1) / 2, iy0 + 3], [ix1 - 3, (iy0 + iy1) / 2]];
    for (const [px, py] of cands) if (P.hits(el, document.elementFromPoint(px, py))) return { ...m, px, py, state: full ? 'ok' : 'partiel' };
    return { ...m, state: 'masqué' };
  };
  P.info = (el) => {
    const tag = el.getAttribute('role') || (el.tagName.toLowerCase() + (el.type && el.tagName === 'INPUT' ? ':' + el.type : '')) || 'cliquable';
    const flags = [];
    if (el.disabled || el.getAttribute('aria-disabled') === 'true') flags.push('désactivé');
    if (el.getAttribute('aria-checked') === 'true' || el.checked) flags.push('coché');
    if (el.getAttribute('aria-pressed') === 'true' || el.getAttribute('aria-selected') === 'true' || (el.getAttribute('aria-current') && el.getAttribute('aria-current') !== 'false')) flags.push('actif');
    if (el.getAttribute('aria-expanded') === 'true') flags.push('ouvert');
    let val = '';
    if (el.tagName === 'SELECT') val = el.selectedOptions[0]?.label || '';
    else if ((el.tagName === 'INPUT' && !/^(checkbox|radio|button|submit)$/.test(el.type)) || el.tagName === 'TEXTAREA') val = el.value;
    return { tag, label: P.label(el), flags, val: P.norm(val).slice(0, 40) };
  };
  P.collect = (clip) => {
    const set = new Set(document.querySelectorAll(SEL));
    // Éléments cliquables sans rôle (cartes, lignes…) : curseur main, le plus haut de sa lignée.
    for (const el of document.body ? document.body.querySelectorAll('div,li,tr,td,span,img,svg,label,h1,h2,h3,h4,p') : []) {
      if (set.has(el) || el.closest(SEL)) continue;
      if (getComputedStyle(el).cursor !== 'pointer') continue;
      if (el.parentElement && getComputedStyle(el.parentElement).cursor === 'pointer') continue;
      if (el.querySelector(SEL)) continue;
      set.add(el);
    }
    P.els = [];
    const out = [];
    for (const el of set) {
      if (el.closest('[inert],[aria-hidden=true]') && !el.matches('input[type=checkbox],input[type=radio]')) continue;
      if (el.matches('[tabindex]') && !el.matches(SEL.replace(",[tabindex]:not([tabindex='-1'])", '')) && el.querySelector(SEL)) continue;
      const m = P.measure(el, clip);
      if (!m) continue;
      const i = P.els.push(el) - 1;
      out.push({ i, ...P.info(el), ...m, tag: el.matches(SEL) ? P.info(el).tag : 'cliquable' });
    }
    return out;
  };
  P.get = (i) => { const el = P.els[i]; return el && el.isConnected ? el : null; };
  // Fait défiler l'élément dans la vue puis le remesure. mode : 'force' (toujours), 'clic' (s'il
  // n'est pas cliquable, ex. sous un en-tête collant), 'ecran' (s'il est hors écran), 'non'.
  P.reveal = async (i, clip, mode) => {
    const el = P.get(i);
    if (!el) return null;
    let m = P.measure(el, clip);
    const go = m && (mode === 'force' || (mode === 'clic' && m.state !== 'ok') || (mode === 'ecran' && (m.state === 'hors écran' || m.state === 'partiel')));
    if (go) {
      const target = (P.box(el).width < 2 && el.labels && el.labels[0]) || el;
      target.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      m = P.measure(el, clip);
    }
    return m && { ...m, ...P.info(el) };
  };
  // Zone défilante principale : celle sous le centre de la zone visible, sinon la plus grande.
  P.scroller = (clip) => {
    const ok = (e) => e && e.scrollHeight > e.clientHeight + 1 && (e === document.scrollingElement || /auto|scroll|overlay/.test(getComputedStyle(e).overflowY));
    let e = document.elementFromPoint((clip.x0 + clip.x1) / 2, (clip.y0 + clip.y1) / 2);
    while (e && !ok(e)) e = e.parentElement;
    if (e) return e;
    if (ok(document.scrollingElement)) return document.scrollingElement;
    return [...document.querySelectorAll('*')].filter(ok).sort((a, b) => b.clientHeight * b.clientWidth - a.clientHeight * a.clientWidth)[0] || null;
  };
  P.scroll = async (how, clip) => {
    const e = P.scroller(clip);
    if (!e) return 'aucune zone défilante';
    const before = Math.round(e.scrollTop);
    const top = how === 'haut' ? 0 : how === 'bas' ? e.scrollHeight : e.scrollTop + Number(how);
    e.scrollTo({ top, behavior: 'instant' });
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    const after = Math.round(e.scrollTop), max = e.scrollHeight - e.clientHeight;
    const name = e === document.scrollingElement ? 'page' : e.tagName.toLowerCase() + (e.id ? '#' + e.id : '') + (e.getAttribute('aria-label') ? ' « ' + e.getAttribute('aria-label') + ' »' : '');
    return (after === before ? 'déjà ' + (after <= 0 ? 'en haut' : after >= max - 1 ? 'en bas' : 'immobile') + ' : ' : 'défilé : ') + name + ' ' + before + ' → ' + after + ' / ' + Math.round(max) + ' px';
  };
  // Collecteur de console : garde les erreurs du cadre jusqu'au prochain rechargement.
  if (!window.__piloteConsole) {
    const buf = (window.__piloteConsole = []);
    const fmt = (a) => a.map((x) => (x instanceof Error ? x.stack || x.message : typeof x === 'object' ? (() => { try { return JSON.stringify(x); } catch { return String(x); } })() : String(x))).join(' ').slice(0, 2000);
    const push = (k, a) => { buf.push(new Date().toTimeString().slice(0, 8) + ' [' + k + '] ' + fmt(a)); if (buf.length > 300) buf.shift(); };
    for (const k of ['error', 'warn']) { const o = console[k].bind(console); console[k] = (...a) => { push(k === 'warn' ? 'warning' : k, a); o(...a); }; }
    addEventListener('error', (e) => push('exception', [e.error || e.message]));
    addEventListener('unhandledrejection', (e) => push('rejet', [e.reason]));
  }
})()`;

async function inject(f, timeout) {
  await evalIn(f, LIB, timeout);
}

// Points de l'iframe réellement atteignables depuis la page (rien ne la recouvre).
async function ownerHits(f, pts) {
  if (!f.owner || !pts.length) return pts.map(() => true);
  const parent = f.owner;
  const r = await send(
    "Runtime.callFunctionOn",
    { objectId: parent.objectId, returnByValue: true, arguments: [{ value: pts }], functionDeclaration: "function (pts) { return pts.map(([x, y]) => { const h = document.elementFromPoint(x, y); return h === this; }); }" },
    parent.session,
  ).catch(() => null);
  return r?.result?.value ?? pts.map(() => true);
}

const frameErrors = [];
async function collect(filter) {
  const list = [];
  frameErrors.length = 0;
  for (const f of pickFrames(await framesCached(), filter)) {
    let els;
    try {
      await inject(f, 8000);
      els = await evalIn(f, `window.__pilote.collect(${JSON.stringify(f.clip)})`, 10_000);
    } catch (err) {
      frameErrors.push(`${f.where} illisible : ${err.message.split("\n")[0]}`);
      continue;
    }
    // Une iframe peut être recouverte dans la page (fenêtre modale de l'app) : test dans le parent.
    const okEls = els.filter((e) => e.px !== undefined);
    const own = offsetOfOwnerFrame(f);
    const hits = await ownerHits(f, okEls.map((e) => [e.px + f.off.x - own.x, e.py + f.off.y - own.y]));
    okEls.forEach((e, k) => {
      if (!hits[k]) e.state = "masqué";
    });
    for (const e of els) list.push({ ...e, frame: f.key, where: f.where, X: e.x + f.off.x, Y: e.y + f.off.y, PX: e.px !== undefined ? e.px + f.off.x : undefined, PY: e.py !== undefined ? e.py + f.off.y : undefined });
  }
  list.forEach((e, k) => (e.n = k + 1));
  fs.writeFileSync(STATE, JSON.stringify(list));
  return list;
}
// Décalage du cadre qui contient l'iframe (0 pour une iframe posée dans la page).
let framesCache = [];
function offsetOfOwnerFrame(f) {
  const p = framesCache.find((x) => x.key === f.parentKey);
  return p?.off ?? { x: 0, y: 0 };
}
async function framesCached() {
  framesCache = await frames();
  return framesCache;
}

function remembered() {
  if (!fs.existsSync(STATE)) throw new Error("Lancez d'abord « voir ».");
  return JSON.parse(fs.readFileSync(STATE, "utf8"));
}

const describe = (e) => `#${e.n} « ${e.label || e.tag} » (${e.where})`;

// Cible : #n de « voir », ou texte (exact > début > inclusion ; plugin affiché > app ; à l'écran d'abord).
async function resolveTarget(spec, rank = 0) {
  if (/^#\d+$/.test(spec)) {
    const e = remembered().find((x) => x.n === Number(spec.slice(1)));
    if (!e) throw new Error(`Élément ${spec} inconnu : relancez « voir ».`);
    return e;
  }
  const want = norm(spec);
  const all = await collect(frameFilter);
  const score = (e) => {
    const l = norm(e.label) + (e.val ? "" : "");
    const tier = l === want ? 0 : l.startsWith(want) ? 1 : l.includes(want) || norm(e.val) === want ? 2 : -1;
    if (tier < 0) return null;
    return tier * 10 + (e.where === "app" ? 1 : 0) + (e.state === "ok" ? 0 : e.state === "partiel" ? 2 : e.state === "hors écran" ? 4 : 6) * 100;
  };
  const found = all
    .map((e) => ({ e, s: score(e) }))
    .filter((x) => x.s !== null)
    .sort((a, b) => a.s - b.s || a.e.n - b.e.n)
    .map((x) => x.e);
  if (!found[rank]) throw new Error(`Aucun élément « ${spec} »${frameFilter ? ` dans ${frameFilter}` : ""} (rang ${rank}, ${found.length} trouvé(s)).`);
  const e = found[rank];
  if (found.length > 1) e.note = `${found.length} correspondances, choisi ${describe(e)} ; autres : ${found.filter((x) => x !== e).slice(0, 4).map(describe).join(", ")}`;
  return e;
}

// Retrouve le cadre et l'élément vivant ; fait défiler dans la vue ; renvoie le point de clic absolu.
async function reveal(e, mode = "clic") {
  const all = await framesCached();
  let f = all.find((x) => x.key === e.frame);
  if (!f) throw new Error(`Le cadre de ${describe(e)} n'existe plus : relancez « voir ».`);
  await inject(f);
  // L'iframe elle-même doit être dans la vue de la page.
  if (f.owner && !f.visible) throw new Error(`Le cadre ${f.where} n'est pas affiché.`);
  let m = await evalIn(f, `window.__pilote.reveal(${e.i}, ${JSON.stringify(f.clip)}, ${JSON.stringify(mode)})`);
  if (!m) {
    // Élément remplacé (nouveau rendu) : on le retrouve par son libellé dans le même cadre.
    const fresh = (await collect()).filter((x) => x.frame === e.frame && x.tag === e.tag && norm(x.label) === norm(e.label));
    if (fresh.length !== 1) throw new Error(`${describe(e)} a disparu de l'écran : relancez « voir ».`);
    e = { ...fresh[0], note: `numérotation rafraîchie : ${describe(fresh[0])}` };
    f = (await framesCached()).find((x) => x.key === e.frame);
    m = await evalIn(f, `window.__pilote.reveal(${e.i}, ${JSON.stringify(f.clip)}, ${JSON.stringify(mode)})`);
    if (!m) throw new Error(`${describe(e)} introuvable.`);
  }
  // Le défilement a pu déplacer l'iframe : on remesure le cadre.
  const f2 = (await framesCached()).find((x) => x.key === e.frame) ?? f;
  const res = { ...e, ...m, X: m.x + f2.off.x, Y: m.y + f2.off.y, f: f2 };
  if (m.px !== undefined) {
    const own = offsetOfOwnerFrame(f2);
    const [hit] = await ownerHits(f2, [[m.px + f2.off.x - own.x, m.py + f2.off.y - own.y]]);
    if (!hit) res.state = "masqué";
    res.PX = m.px + f2.off.x;
    res.PY = m.py + f2.off.y;
  }
  return res;
}

async function mouseClick(x, y) {
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
}

async function click(e) {
  const r = await reveal(e);
  if (r.state === "hors écran" || r.state === "masqué" || r.PX === undefined)
    throw new Error(`${describe(r)} n'est pas cliquable (${r.state}) : ${r.state === "masqué" ? "recouvert par un autre élément (fenêtre modale, menu ?)" : "impossible de l'amener à l'écran"}.`);
  // Vrai clic souris aux coordonnées de l'élément (comme un utilisateur), pas element.click().
  await mouseClick(r.PX, r.PY);
  return { r, text: `cliqué ${describe(r)}${r.flags?.includes("désactivé") ? " [désactivé]" : ""}${e.note ? `\n(${e.note})` : ""}` };
}

const KEYS = { Enter: 13, Escape: 27, Tab: 9, Backspace: 8, ArrowDown: 40, ArrowUp: 38, ArrowLeft: 37, ArrowRight: 39, Delete: 46, Space: 32, " ": 32, Home: 36, End: 35, PageDown: 34, PageUp: 33 };
async function key(k, mods = 0) {
  const name = k === "Space" ? " " : k;
  for (const type of ["keyDown", "keyUp"])
    await send("Input.dispatchKeyEvent", {
      type,
      key: name,
      code: k === "Space" ? "Space" : k.length === 1 ? "Key" + k.toUpperCase() : k,
      windowsVirtualKeyCode: KEYS[k] ?? (k.length === 1 ? k.toUpperCase().charCodeAt(0) : 0),
      modifiers: mods,
      ...(type === "keyDown" && k === "Enter" ? { text: "\r" } : type === "keyDown" && name === " " ? { text: " " } : {}),
    });
}

// ── Capture annotée ──────────────────────────────────────────────────────────────────────────
async function capture(file, boxes, note) {
  const marks = [];
  for (const b of boxes) {
    if (/^-?\d+(\.\d+)?(,-?\d+(\.\d+)?){3}$/.test(b)) {
      const [x, y, w, h] = b.split(",").map(Number);
      marks.push({ x, y, w, h });
    } else {
      const e = await resolveTarget(b);
      await reveal(e, "ecran");
      marks.push({ e });
    }
  }
  // Remesure après tous les défilements : amener un élément peut en déplacer un autre.
  for (const m of marks)
    if (m.e) {
      const r = await reveal(m.e, "non");
      Object.assign(m, { x: r.X, y: r.Y, w: r.w, h: r.h });
      delete m.e;
    }
  const app = framesCache.find((f) => f.where === "app") ?? (await framesCached()).find((f) => f.where === "app");
  const overlay = `(() => {
    const root = document.createElement('div');
    root.id = '__pilote_marques';
    root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:2147483647';
    document.body.appendChild(root);
    const marks = ${JSON.stringify(marks)};
    const note = ${JSON.stringify(note ?? "")};
    const W = innerWidth, H = innerHeight, G = 10;
    marks.forEach((m) => {
      const d = document.createElement('div');
      d.style.cssText = 'position:fixed;box-sizing:border-box;border:3px solid #ff2d2d;border-radius:6px;box-shadow:0 0 0 3px rgba(255,45,45,.25);left:' + (m.x - 5) + 'px;top:' + (m.y - 5) + 'px;width:' + (m.w + 10) + 'px;height:' + (m.h + 10) + 'px';
      root.appendChild(d);
    });
    if (note && marks.length) {
      const m = marks[0];
      const n = document.createElement('div');
      n.textContent = note;
      n.style.cssText = 'position:fixed;box-sizing:border-box;max-width:' + Math.min(440, W - 16) + 'px;background:#ff2d2d;color:#fff;font:600 13px/1.4 system-ui;padding:6px 10px;border-radius:6px;box-shadow:0 2px 8px rgba(0,0,0,.3)';
      root.appendChild(n);
      const nw = n.offsetWidth, nh = n.offsetHeight;
      const L = m.x - 5, T = m.y - 5, R = m.x + m.w + 5, B = m.y + m.h + 5;
      const clampX = (x) => Math.max(4, Math.min(W - nw - 4, x));
      const clampY = (y) => Math.max(4, Math.min(H - nh - 4, y));
      let x, y;
      if (B + G + nh <= H - 4) { x = clampX(L); y = B + G; }              // dessous
      else if (T - G - nh >= 4) { x = clampX(L); y = T - G - nh; }         // dessus
      else if (R + G + nw <= W - 4) { x = R + G; y = clampY(T); }          // à droite
      else if (L - G - nw >= 4) { x = L - G - nw; y = clampY(T); }         // à gauche
      else { x = clampX(L + 8); y = clampY(T + 8); n.style.outline = '2px solid #fff'; } // cadre plein écran : à l'intérieur
      n.style.left = x + 'px';
      n.style.top = y + 'px';
    }
  })()`;
  if (marks.length) await evalIn(app, overlay);
  await sleep(60);
  const res = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  if (marks.length) await evalIn(app, `document.getElementById('__pilote_marques')?.remove()`);
  fs.mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  fs.writeFileSync(file, Buffer.from(res.data, "base64"));
  return `capture : ${file}` + (marks.length ? ` (${marks.length} cadre(s))` : "");
}

// ── Taille de la fenêtre (Win32) ─────────────────────────────────────────────────────────────
function win32(action, w = 0, h = 0) {
  const exe = path.join(REPO, "src-tauri", "target-agents", "debug", "cadence.exe");
  const ps = `
$ErrorActionPreference = 'Stop'
Add-Type -Namespace P -Name W -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
[DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
public struct RECT { public int L, T, R, B; }
'@
[void][P.W]::SetProcessDPIAware()
$p = Get-Process cadence -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${exe.replace(/'/g, "''")}' -and $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { 'ABSENT'; exit }
$h = $p.MainWindowHandle
switch ('${action}') {
  'max' { [void][P.W]::ShowWindow($h, 3) }
  'restore' { if ([P.W]::IsZoomed($h)) { [void][P.W]::ShowWindow($h, 9) } }
  'set' { [void][P.W]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, ${Math.round(w)}, ${Math.round(h)}, 0x16) }
}
Start-Sleep -Milliseconds 150
$r = New-Object P.W+RECT; [void][P.W]::GetWindowRect($h, [ref]$r)
'{0} {1} {2}' -f ($r.R - $r.L), ($r.B - $r.T), [int][P.W]::IsZoomed($h)`;
  const r = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", ps], { encoding: "utf8" });
  const line = (r.stdout ?? "").trim().split(/\r?\n/).pop();
  if (line === "ABSENT") throw new Error("fenêtre de Cadence QA introuvable (cadence.exe de target-agents).");
  const [ww, wh, zoomed] = (line ?? "").split(" ").map(Number);
  if (!ww) throw new Error("redimensionnement impossible : " + (r.stderr || r.stdout || "").trim().slice(0, 300));
  return { ww, wh, zoomed: zoomed === 1 };
}

async function viewport() {
  const app = (await framesCached()).find((f) => f.where === "app");
  return evalIn(app, "[innerWidth, innerHeight, devicePixelRatio]");
}

// ── Commandes ────────────────────────────────────────────────────────────────────────────────
let out;
try {
  switch (cmd) {
    case "voir": {
      const list = await collect(frameFilter);
      const shown = list.filter((e) => opt.tout || e.state === "ok" || e.state === "partiel");
      const lines = shown.map((e) => {
        const flags = [...e.flags, ...(e.state === "ok" ? [] : [e.state])];
        return `#${e.n}\t${e.where}\t${e.tag}\t${e.label || "(sans libellé)"}${e.val ? ` = « ${e.val} »` : ""}${flags.length ? "  [" + flags.join(", ") + "]" : ""}`;
      });
      const hidden = {};
      for (const e of list) if (!shown.includes(e)) hidden[e.state] = (hidden[e.state] ?? 0) + 1;
      const extra = Object.entries(hidden).map(([s, n]) => `${n} ${s}`);
      const frs = framesCache.filter((f) => f.where !== "app");
      out =
        (lines.join("\n") || "aucun élément cliquable à l'écran (écran en cours de chargement ? relancer « voir »)") +
        (frameErrors.length ? `\n(${frameErrors.join(" ; ")})` : "") +
        (extra.length ? `\n(+ ${extra.join(", ")} : « voir --tout » pour les lister, « defiler » pour les amener)` : "") +
        (frs.length ? `\n(cadres : app${frs.map((f) => `, ${f.where}${f.visible ? " affiché" : " caché"}`).join("")})` : "");
      break;
    }
    case "texte": {
      const max = Number(pos[0] ?? 6000);
      const delai = (opt.delai ?? 5) * 1000;
      const parts = [];
      for (const f of pickFrames(await framesCached(), frameFilter)) {
        try {
          const t = await evalIn(f, "document.body ? document.body.innerText : ''", delai);
          if (t?.trim()) parts.push(`── ${f.where} ──\n${t.trim()}`);
        } catch (err) {
          parts.push(`── ${f.where} ── (illisible : ${err.message})`);
        }
      }
      out = parts.join("\n\n").slice(0, max) || "aucun texte";
      break;
    }
    case "clic": {
      out = (await click(await resolveTarget(pos[0], Number(pos[1] ?? 0)))).text;
      break;
    }
    case "saisir": {
      const e = await resolveTarget(pos[0]);
      const value = pos.slice(1).join(" ");
      const r = await reveal(e);
      const f = r.f;
      const kind = await evalIn(f, `(() => { const el = window.__pilote.get(${r.i}); return el ? (el.tagName === 'SELECT' ? 'select' : /^(checkbox|radio)$/.test(el.type) || /checkbox|radio|switch/.test(el.getAttribute('role') || '') ? 'case' : 'texte') : null; })()`);
      if (kind === "case") throw new Error(`${describe(r)} est une case : utilisez « cocher ».`);
      if (kind === "select") {
        // Liste native : son menu déroulant n'est pas pilotable à la souris, on choisit l'option comme le ferait le clavier.
        const res = await evalIn(
          f,
          `(() => {
          const el = window.__pilote.get(${r.i}), want = ${JSON.stringify(norm(value))};
          const n = (s) => (s || '').replace(/\\s+/g, ' ').trim().toLowerCase();
          const opts = [...el.options];
          const o = opts.find((o) => n(o.label) === want) || opts.find((o) => n(o.value) === want) || opts.find((o) => n(o.label).startsWith(want)) || opts.find((o) => n(o.label).includes(want));
          if (!o) return { err: opts.map((o) => o.label).join(' | ') };
          if (o.disabled) return { err: 'option « ' + o.label + ' » désactivée' };
          el.focus();
          Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(el, o.value);
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return { ok: o.label };
        })()`,
        );
        if (res.err) throw new Error(`option « ${value} » absente de ${describe(r)} (options : ${res.err}).`);
        out = `choisi « ${res.ok} » dans ${describe(r)}`;
        break;
      }
      if (r.state === "hors écran" || r.state === "masqué" || r.PX === undefined) throw new Error(`${describe(r)} n'est pas atteignable (${r.state}).`);
      await mouseClick(r.PX, r.PY);
      await evalIn(
        f,
        `(() => { const el = window.__pilote.get(${r.i}); if (!el) return; el.focus();
          if (el.isContentEditable) { const s = getSelection(), rg = document.createRange(); rg.selectNodeContents(el); s.removeAllRanges(); s.addRange(rg); }
          else el.select?.(); })()`,
      );
      if (value) await send("Input.insertText", { text: value });
      else await key("Delete");
      await sleep(50);
      const got = await evalIn(f, `(() => { const el = window.__pilote.get(${r.i}); return el ? (el.isContentEditable ? el.innerText : el.value) : null; })()`);
      out = `saisi dans ${describe(r)}` + (norm(got) === norm(value) ? "" : ` — attention, le champ contient « ${(got ?? "").slice(0, 80)} »`) + (e.note ? `\n(${e.note})` : "");
      break;
    }
    case "cocher": {
      const e = await resolveTarget(pos[0]);
      const want = !/^(non|false|0|décocher)$/i.test(pos[1] ?? "oui");
      const state = `(() => { const el = window.__pilote.get(${e.i}); if (!el) return null; return el.type === 'checkbox' || el.type === 'radio' ? el.checked : el.getAttribute('aria-checked') === 'true' || el.getAttribute('aria-pressed') === 'true'; })()`;
      const f = (await framesCached()).find((x) => x.key === e.frame);
      const before = await evalIn(f, state);
      if (before === want) {
        out = `${describe(e)} déjà ${want ? "coché" : "décoché"}`;
        break;
      }
      const { r } = await click(e);
      await sleep(150);
      const after = await evalIn(r.f, state);
      out = after === want ? `${describe(r)} ${want ? "coché" : "décoché"}` : `cliqué ${describe(r)} mais l'état n'a pas changé (${after ? "coché" : "décoché"})`;
      break;
    }
    case "taper":
      await send("Input.insertText", { text: pos.join(" ") });
      out = "tapé";
      break;
    case "touche": {
      const n = Number(pos[1] ?? 1);
      const [k, ...m] = pos[0].split("+").reverse();
      const mods = m.reduce((a, x) => a | ({ alt: 1, ctrl: 2, meta: 4, shift: 8 }[x.toLowerCase()] ?? 0), 0);
      for (let i = 0; i < n; i++) await key(k, mods);
      out = "touche " + pos[0] + (n > 1 ? ` ×${n}` : "");
      break;
    }
    case "defiler": {
      const spec = pos[0] ?? "bas";
      if (/^(haut|bas|[+-]\d+)$/.test(spec)) {
        // Cadre actif : le plugin affiché s'il y en a un, sinon l'app.
        const cands = pickFrames(await framesCached(), frameFilter);
        const f = cands.find((x) => x.where !== "app") ?? cands[0];
        await inject(f);
        out = `${f.where} — ` + (await evalIn(f, `window.__pilote.scroll(${JSON.stringify(spec)}, ${JSON.stringify(f.clip)})`));
      } else {
        const e = await resolveTarget(spec, Number(pos[1] ?? 0));
        const r = await reveal(e, "force");
        out = `${describe(r)} amené à l'écran (${r.state})${e.note ? `\n(${e.note})` : ""}`;
      }
      break;
    }
    case "attendre": {
      const want = norm(pos[0]);
      const secs = Number(pos[1] ?? 30);
      const limit = Date.now() + secs * 1000;
      out = opt.absent ? `« ${pos[0]} » toujours là après ${secs} s` : `« ${pos[0]} » absent après ${secs} s`;
      process.exitCode = 1;
      while (Date.now() < limit) {
        let seen = false;
        for (const f of pickFrames(await framesCached(), frameFilter)) {
          try {
            if (norm(await evalIn(f, "document.body ? document.body.innerText : ''", 5000)).includes(want)) seen = true;
          } catch {}
        }
        if (seen !== !!opt.absent) {
          out = opt.absent ? `« ${pos[0]} » disparu` : `« ${pos[0]} » visible`;
          process.exitCode = 0;
          break;
        }
        await sleep(1000);
      }
      break;
    }
    case "capture": {
      out = await capture(pos[0], opt.cadres, opt.note);
      break;
    }
    case "eval": {
      const [f] = pickFrames(await framesCached(), frameFilter ?? "app");
      out = await evalIn(f, `(async () => (${pos.join(" ")}))()`);
      break;
    }
    case "console": {
      if (opt.ecouter) {
        const secs = Number(pos[0] ?? 3600);
        fs.appendFileSync(CONSOLE_FILE, `${new Date().toTimeString().slice(0, 8)} [pilote] écoute démarrée (${secs} s)\n`);
        consoleLines.length = 0; // la relecture initiale est déjà couverte par « console »
        let written = 0;
        const flush = () => {
          if (consoleLines.length > written) {
            const chunk = consoleLines.slice(written).join("\n") + "\n";
            fs.appendFileSync(CONSOLE_FILE, chunk);
            process.stdout.write(chunk);
            written = consoleLines.length;
          }
        };
        const end = Date.now() + secs * 1000;
        while (Date.now() < end && ws.readyState === 1) {
          await sleep(500);
          flush();
        }
        out = `écoute terminée (${written} message(s) dans ${CONSOLE_FILE})`;
        break;
      }
      for (const s of sessions.values()) await send("Log.enable", {}, s.id).catch(() => {});
      await sleep(700);
      const fs_ = await framesCached();
      const collected = [];
      for (const f of fs_) {
        try {
          await inject(f, 5000);
          const lines = await evalIn(f, opt.effacer ? "window.__piloteConsole.splice(0)" : "window.__piloteConsole.slice()", 5000);
          for (const l of lines ?? []) collected.push(`${f.where} ${l}`);
        } catch {}
      }
      const replay = consoleLines.map((l) => l.slice(9)); // sans l'heure de réception (relecture)
      const seen = new Set();
      const all = [];
      for (const l of [...replay.map((l) => `(relu) ${l}`), ...collected]) {
        const k = l.replace(/^\(relu\) /, "").replace(/\d\d:\d\d:\d\d /, "").replace(/\[warn(ing)?\]/, "[warning]").slice(0, 200);
        if (seen.has(k)) continue;
        seen.add(k);
        all.push(l);
      }
      let buffered = "";
      if (fs.existsSync(CONSOLE_FILE)) {
        buffered = fs.readFileSync(CONSOLE_FILE, "utf8").trim().split("\n").slice(-60).join("\n");
        if (opt.effacer) fs.rmSync(CONSOLE_FILE);
      }
      out =
        (all.join("\n") || "aucune erreur ni avertissement depuis le chargement des cadres") +
        (buffered ? `\n── écoute continue (${CONSOLE_FILE}) ──\n${buffered}` : "") +
        (opt.effacer ? "\n(tampons vidés)" : "");
      break;
    }
    case "taille": {
      if (!pos.length) {
        const [w, h, dpr] = await viewport();
        out = `rendu ${w}×${h} px CSS (échelle ${dpr})`;
        break;
      }
      if (pos[0] === "max") {
        win32("max");
        await sleep(400);
        const [w, h] = await viewport();
        out = `fenêtre agrandie : rendu ${w}×${h}`;
        break;
      }
      const [W, H] = pos.map(Number);
      if (!(W > 0 && H > 0)) throw new Error("usage : taille <largeur> <hauteur> | taille max");
      win32("restore");
      await sleep(300);
      // Écart fenêtre ↔ zone de rendu (bordures, barre de titre), en pixels physiques.
      for (let i = 0; i < 3; i++) {
        const [w, h, dpr] = await viewport();
        if (Math.abs(w - W) <= 1 && Math.abs(h - H) <= 1) break;
        const cur = win32("info");
        win32("set", cur.ww + (W - w) * dpr, cur.wh + (H - h) * dpr);
        await sleep(400);
      }
      const [w, h] = await viewport();
      out = `rendu ${w}×${h} px CSS` + (w !== W || h !== H ? ` (demandé ${W}×${H} : taille minimale de la fenêtre ?)` : "") + " — « taille max » pour revenir";
      break;
    }
    default:
      out = "Commandes : voir, texte, clic, saisir, cocher, taper, touche, defiler, attendre, capture, eval, console, taille (détail en tête de cdp.mjs)";
  }
} catch (err) {
  out = "Erreur : " + err.message;
  process.exitCode = 1;
}
console.log(typeof out === "string" ? out : JSON.stringify(out, null, 1));
ws.close();
process.exit();
