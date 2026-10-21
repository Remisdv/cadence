// Pilote CDP de la WebView2 de Cadence, pour les tests en conditions réelles.
// Aucune dépendance (Node ≥ 22 : fetch et WebSocket natifs).
//
//   node cdp.mjs voir                         éléments interactifs visibles, numérotés (#n), tous cadres
//   node cdp.mjs texte [max]                  texte visible de la page et des cadres de plugin
//   node cdp.mjs clic <#n | texte> [rang]     clique l'élément #n de « voir », ou le rang-ième contenant ce texte
//   node cdp.mjs saisir <#n | texte> <valeur> remplit un champ (input, textarea, contenteditable)
//   node cdp.mjs taper <texte>                insère du texte dans l'élément qui a le focus
//   node cdp.mjs touche <Enter|Escape|Tab|…>  envoie une touche
//   node cdp.mjs attendre <texte> [secondes]  attend qu'un texte apparaisse (défaut 30 s)
//   node cdp.mjs capture <fichier.png> [--cadre #n|x,y,l,h]... [--note "texte"]
//                                             capture ; chaque --cadre entoure une zone en rouge,
//                                             --note ajoute une étiquette sous le premier cadre
//   node cdp.mjs eval <expression>            évalue dans la page (diagnostic seulement)
//   node cdp.mjs console                      erreurs et avertissements console depuis le lancement du pilote
//   node cdp.mjs taille <largeur> <hauteur>   force la taille de la fenêtre de rendu (0 0 = rétablir)
//
// Variables : CDP_PORT (défaut 9339). « voir » mémorise la numérotation dans .cdp-elements.json
// (dossier courant) ; clic/saisir/capture --cadre #n la relisent : relancer « voir » après chaque
// changement d'écran.
import fs from "node:fs";
import path from "node:path";

const PORT = process.env.CDP_PORT ?? "9339";
const STATE = path.resolve(".cdp-elements.json");
const [cmd, ...args] = process.argv.slice(2);

const targets = await fetch(`http://127.0.0.1:${PORT}/json/list`)
  .then((r) => r.json())
  .catch(() => {
    console.error(`Pas de WebView2 en débogage sur le port ${PORT} : lancez d'abord scripts/lancer.ps1.`);
    process.exit(2);
  });
const page = targets.find((t) => t.type === "page" && !t.url.startsWith("devtools"));
if (!page) {
  console.error("Aucune page Cadence trouvée.");
  process.exit(2);
}

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, ko) => {
  ws.onopen = ok;
  ws.onerror = ko;
});
let seq = 0;
const pending = new Map();
const consoleLines = [];
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg);
    pending.delete(msg.id);
  } else if (msg.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(msg.params.type)) {
    consoleLines.push(`[${msg.params.type}] ` + msg.params.args.map((a) => a.value ?? a.description ?? "").join(" "));
  } else if (msg.method === "Runtime.exceptionThrown") {
    consoleLines.push("[exception] " + (msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text));
  }
};
const send = (method, params = {}) =>
  new Promise((ok) => {
    const id = ++seq;
    pending.set(id, ok);
    ws.send(JSON.stringify({ id, method, params }));
  });

// Cadres : la page principale et les iframes des plugins (DOM partagé via un monde isolé).
async function frames() {
  const tree = (await send("Page.getFrameTree")).result.frameTree;
  const all = [];
  const walk = (node, depth) => {
    all.push({ ...node.frame, depth });
    (node.childFrames ?? []).forEach((c) => walk(c, depth + 1));
  };
  walk(tree, 0);
  const out = [];
  for (const f of all) {
    const ctx = f.depth === 0 ? undefined : (await send("Page.createIsolatedWorld", { frameId: f.id, worldName: "pilote" })).result?.executionContextId;
    out.push({ id: f.id, url: f.url, depth: f.depth, ctx });
  }
  return out;
}

async function evaluate(expression, ctx) {
  const res = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
    ...(ctx ? { contextId: ctx } : {}),
  });
  if (res.result?.exceptionDetails) throw new Error(res.result.exceptionDetails.exception?.description ?? res.result.exceptionDetails.text);
  return res.result?.result?.value;
}

// Position d'une iframe de plugin dans la page principale (pour les cadres de capture).
async function iframeOffset(frame) {
  if (frame.depth === 0) return { x: 0, y: 0 };
  const owner = (await send("DOM.getFrameOwner", { frameId: frame.id })).result;
  if (!owner?.backendNodeId) return { x: 0, y: 0 };
  const box = (await send("DOM.getBoxModel", { backendNodeId: owner.backendNodeId })).result;
  if (!box) return { x: 0, y: 0 };
  return { x: box.model.content[0], y: box.model.content[1] };
}

const COLLECT = `(() => {
  const sel = 'button,a[href],[role=button],[role=menuitem],[role=menuitemradio],[role=tab],[role=checkbox],[role=radio],[role=option],[role=switch],[role=link],input,textarea,select,[contenteditable=true],summary';
  const out = [];
  for (const el of document.querySelectorAll(sel)) {
    const r = el.getBoundingClientRect();
    const st = getComputedStyle(el);
    if (r.width < 2 || r.height < 2 || st.visibility === 'hidden' || st.display === 'none') continue;
    if (r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth) continue;
    const label = (el.getAttribute('aria-label') || (el.labels && el.labels[0] && el.labels[0].innerText) || (el.getAttribute('aria-labelledby') && document.getElementById(el.getAttribute('aria-labelledby'))?.innerText) || el.innerText || el.placeholder || el.title || (el.type === 'radio' || el.type === 'checkbox' ? '' : el.value) || '').replace(/\\s+/g, ' ').trim();
    const tag = el.getAttribute('role') || el.tagName.toLowerCase() + (el.type ? ':' + el.type : '');
    const flags = [el.disabled || el.getAttribute('aria-disabled') === 'true' ? 'désactivé' : '', el.getAttribute('aria-checked') === 'true' || el.checked ? 'coché' : '', el.getAttribute('aria-selected') === 'true' || el.getAttribute('aria-current') ? 'actif' : '', el.getAttribute('aria-expanded') === 'true' ? 'ouvert' : ''].filter(Boolean);
    out.push({ tag, label: label.slice(0, 90), flags, x: r.left, y: r.top, w: r.width, h: r.height });
  }
  return out;
})()`;

async function collect() {
  const list = [];
  for (const f of await frames()) {
    let els;
    try {
      els = await evaluate(COLLECT, f.ctx);
    } catch {
      continue;
    }
    const off = await iframeOffset(f);
    const where = f.depth === 0 ? "app" : "plugin " + (f.url.match(/plugins?[/.]([a-z0-9-]+)/)?.[1] ?? f.url.slice(0, 50));
    els.forEach((e, i) => list.push({ ...e, frame: f.id, where, index: i, X: e.x + off.x, Y: e.y + off.y }));
  }
  list.forEach((e, i) => (e.n = i + 1));
  fs.writeFileSync(STATE, JSON.stringify(list));
  return list;
}

function remembered() {
  if (!fs.existsSync(STATE)) throw new Error("Lancez d'abord « voir ».");
  return JSON.parse(fs.readFileSync(STATE, "utf8"));
}

async function resolveTarget(spec, rank = 0) {
  if (/^#\d+$/.test(spec)) {
    const e = remembered().find((x) => x.n === Number(spec.slice(1)));
    if (!e) throw new Error(`Élément ${spec} inconnu : relancez « voir ».`);
    return e;
  }
  const want = spec.toLowerCase();
  const found = (await collect()).filter((e) => e.label.toLowerCase().includes(want));
  if (!found[rank]) throw new Error(`Aucun élément « ${spec} » (rang ${rank}, ${found.length} trouvé(s)).`);
  return found[rank];
}

const ctxOf = async (frameId) => (await frames()).find((f) => f.id === frameId)?.ctx;
const nth = (e) => `[...document.querySelectorAll('button,a[href],[role=button],[role=menuitem],[role=menuitemradio],[role=tab],[role=checkbox],[role=radio],[role=option],[role=switch],[role=link],input,textarea,select,[contenteditable=true],summary')].filter(el => { const r = el.getBoundingClientRect(); const st = getComputedStyle(el); return r.width >= 2 && r.height >= 2 && st.visibility !== 'hidden' && st.display !== 'none' && !(r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth); })[${e.index}]`;

async function click(e) {
  // Vrai clic souris aux coordonnées de l'élément (comme un utilisateur), pas element.click().
  const x = e.X + e.w / 2;
  const y = e.Y + e.h / 2;
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
  return `cliqué #${e.n} « ${e.label} » (${e.where})`;
}

async function capture(file, boxes, note) {
  const marks = [];
  for (const b of boxes) {
    if (/^#\d+$/.test(b)) {
      const e = remembered().find((x) => x.n === Number(b.slice(1)));
      if (e) marks.push({ x: e.X, y: e.Y, w: e.w, h: e.h });
    } else {
      const [x, y, w, h] = b.split(",").map(Number);
      marks.push({ x, y, w, h });
    }
  }
  const overlay = `(() => {
    const root = document.createElement('div');
    root.id = '__pilote_marques';
    root.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:2147483647';
    const marks = ${JSON.stringify(marks)};
    marks.forEach((m, i) => {
      const d = document.createElement('div');
      d.style.cssText = 'position:fixed;border:3px solid #ff2d2d;border-radius:6px;box-shadow:0 0 0 3px rgba(255,45,45,.25);left:' + (m.x - 4) + 'px;top:' + (m.y - 4) + 'px;width:' + (m.w + 8) + 'px;height:' + (m.h + 8) + 'px';
      root.appendChild(d);
      if (i === 0 && ${JSON.stringify(note ?? "")}) {
        const n = document.createElement('div');
        n.textContent = ${JSON.stringify(note ?? "")};
        n.style.cssText = 'position:fixed;max-width:420px;background:#ff2d2d;color:#fff;font:600 13px/1.4 system-ui;padding:6px 10px;border-radius:6px;left:' + Math.max(4, m.x - 4) + 'px;top:' + Math.min(innerHeight - 60, m.y + m.h + 10) + 'px';
        root.appendChild(n);
      }
    });
    document.body.appendChild(root);
  })()`;
  if (marks.length) await evaluate(overlay);
  const res = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  if (marks.length) await evaluate(`document.getElementById('__pilote_marques')?.remove()`);
  fs.mkdirSync(path.dirname(path.resolve(file)), { recursive: true });
  fs.writeFileSync(file, Buffer.from(res.result.data, "base64"));
  return `capture : ${file}` + (marks.length ? ` (${marks.length} cadre(s))` : "");
}

let out;
try {
  switch (cmd) {
    case "voir": {
      const list = await collect();
      out = list.map((e) => `#${e.n}\t${e.where}\t${e.tag}\t${e.label || "(sans libellé)"}${e.flags.length ? "  [" + e.flags.join(", ") + "]" : ""}`).join("\n") || "aucun élément interactif visible";
      break;
    }
    case "texte": {
      const max = Number(args[0] ?? 6000);
      const parts = [];
      for (const f of await frames()) {
        try {
          const t = await evaluate("document.body ? document.body.innerText : ''", f.ctx);
          if (t?.trim()) parts.push(`── ${f.depth === 0 ? "app" : f.url.slice(0, 80)} ──\n${t.trim()}`);
        } catch {}
      }
      out = parts.join("\n\n").slice(0, max);
      break;
    }
    case "clic":
      out = await click(await resolveTarget(args[0], Number(args[1] ?? 0)));
      break;
    case "saisir": {
      const e = await resolveTarget(args[0]);
      await click(e);
      const ctx = await ctxOf(e.frame);
      await evaluate(`(() => { const el = ${nth(e)}; if (!el) return; if (el.isContentEditable) { document.execCommand('selectAll'); } else { el.select?.(); } })()`, ctx);
      await send("Input.insertText", { text: args[1] ?? "" });
      out = `saisi dans #${e.n} « ${e.label} »`;
      break;
    }
    case "taper":
      await send("Input.insertText", { text: args.join(" ") });
      out = "tapé";
      break;
    case "touche": {
      const key = args[0];
      const codes = { Enter: 13, Escape: 27, Tab: 9, Backspace: 8, ArrowDown: 40, ArrowUp: 38, ArrowLeft: 37, ArrowRight: 39, Delete: 46, Space: 32 };
      for (const type of ["keyDown", "keyUp"])
        await send("Input.dispatchKeyEvent", { type, key, code: key, windowsVirtualKeyCode: codes[key] ?? 0, ...(key === "Enter" && type === "keyDown" ? { text: "\r" } : {}) });
      out = "touche " + key;
      break;
    }
    case "attendre": {
      const want = (args[0] ?? "").toLowerCase();
      const limit = Date.now() + Number(args[1] ?? 30) * 1000;
      out = `« ${args[0]} » absent après ${args[1] ?? 30} s`;
      while (Date.now() < limit) {
        let seen = false;
        for (const f of await frames()) {
          try {
            if ((await evaluate("document.body ? document.body.innerText : ''", f.ctx)).toLowerCase().includes(want)) seen = true;
          } catch {}
        }
        if (seen) {
          out = `« ${args[0]} » visible`;
          break;
        }
        await new Promise((r) => setTimeout(r, 1000));
      }
      break;
    }
    case "capture": {
      const file = args[0];
      const boxes = [];
      let note;
      for (let i = 1; i < args.length; i++) {
        if (args[i] === "--cadre") boxes.push(args[++i]);
        else if (args[i] === "--note") note = args[++i];
      }
      out = await capture(file, boxes, note);
      break;
    }
    case "eval":
      out = await evaluate(`(async () => (${args.join(" ")}))()`);
      break;
    case "console": {
      await send("Runtime.enable");
      await new Promise((r) => setTimeout(r, 1500));
      out = consoleLines.join("\n") || "aucune erreur reçue (seuls les messages émis pendant l'écoute sont vus)";
      break;
    }
    case "taille": {
      const [w, h] = args.map(Number);
      if (!w) await send("Emulation.clearDeviceMetricsOverride");
      else await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 0, mobile: false });
      out = w ? `rendu forcé à ${w}×${h}` : "taille rétablie";
      break;
    }
    default:
      out = "Commandes : voir, texte, clic, saisir, taper, touche, attendre, capture, eval, console, taille";
  }
} catch (err) {
  out = "Erreur : " + err.message;
  process.exitCode = 1;
}
console.log(typeof out === "string" ? out : JSON.stringify(out, null, 1));
ws.close();
