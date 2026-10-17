/**
 * SDK des interfaces de plugin, version 1 (plan Cadence 2 § 3.3) : implémentation de
 * référence du futur paquet `@cadence/plugin-sdk`, chargée par le plugin `exemple` du mode
 * mock. Script classique (pas de module) : une iframe sans origine le charge sans CORS.
 *
 *   await cadence.ready;                       // poignée de main faite
 *   const r = await cadence.call("compteur", { pas: 1 });
 *   cadence.on("route", ({ view, params }) => …);
 *   cadence.navigate("outils"); cadence.notify("Enregistré", "success");
 *
 * Une erreur rejette avec `{ code, message, data }` (`tool_error`, `unavailable`,
 * `method_not_found`, `timeout`…).
 */
(function (global) {
  "use strict";
  var VERSION = "1.0.0";
  var TIMEOUT_MS = 130000;
  var parent = global.parent;
  var seq = 0;
  var pending = {};
  var listeners = {};
  var info = null;

  function SdkError(error) {
    var e = new Error((error && error.message) || "Erreur de l'hôte.");
    e.name = "CadenceError";
    e.code = (error && error.code) || "internal";
    e.data = error && error.data;
    return e;
  }

  function post(message) {
    parent.postMessage(message, "*");
  }

  function request(method, params, timeoutMs) {
    return new Promise(function (resolve, reject) {
      var id = "sdk" + ++seq;
      var timer = setTimeout(function () {
        delete pending[id];
        reject(SdkError({ code: "timeout", message: "L'hôte n'a pas répondu à « " + method + " »." }));
      }, timeoutMs || TIMEOUT_MS);
      pending[id] = { resolve: resolve, reject: reject, timer: timer };
      post({ cadence: 1, kind: "req", id: id, method: method, params: params });
    });
  }

  function applyTheme(theme) {
    var root = global.document && global.document.documentElement;
    if (!root || !theme) return;
    Object.keys(theme).forEach(function (name) {
      root.style.setProperty(name, theme[name]);
    });
  }

  function emit(event, data) {
    (listeners[event] || []).slice().forEach(function (fn) {
      try {
        fn(data);
      } catch (err) {
        setTimeout(function () {
          throw err;
        });
      }
    });
  }

  global.addEventListener("message", function (event) {
    if (event.source !== parent) return;
    var m = event.data;
    if (!m || m.cadence !== 1) return;
    if (m.kind === "res" && pending[m.id]) {
      var p = pending[m.id];
      delete pending[m.id];
      clearTimeout(p.timer);
      if (m.error) p.reject(SdkError(m.error));
      else p.resolve(m.result);
    } else if (m.kind === "evt") {
      if (m.event === "route" && info) info.route = m.data;
      if (m.event === "theme") applyTheme(m.data);
      emit(m.event, m.data);
    }
  });

  // Poignée de main : les autres appels attendent qu'elle soit faite.
  var ready = request("hello", { sdk: VERSION }, 10000).then(function (result) {
    info = result;
    applyTheme(result.theme);
    return result;
  });

  function call(method, params) {
    return ready.then(function () {
      return request(method, params);
    });
  }

  // Raccourcis réservés au shell (palette de commandes).
  global.addEventListener("keydown", function (event) {
    if ((event.ctrlKey || event.metaKey) && (event.key === "k" || event.key === "K")) {
      event.preventDefault();
      call("ui.shortcut", { key: "k" });
    }
  });

  global.cadence = {
    version: VERSION,
    ready: ready,
    get plugin() {
      return info && info.plugin;
    },
    get route() {
      return info && info.route;
    },
    get theme() {
      return info && info.theme;
    },
    supports: function (method) {
      return !!info && info.methods.indexOf(method) >= 0;
    },
    call: function (tool, args) {
      return call("tools.call", { tool: tool, args: args || null });
    },
    on: function (event, fn) {
      (listeners[event] = listeners[event] || []).push(fn);
      return function () {
        listeners[event] = (listeners[event] || []).filter(function (f) {
          return f !== fn;
        });
      };
    },
    navigate: function (view, params) {
      return call("nav.go", { view: view, params: params || {} });
    },
    notify: function (message, kind) {
      return call("ui.notify", { message: message, kind: kind || "info" });
    },
    openExternal: function (url) {
      return call("ui.open", { url: url });
    },
    runAction: function (id, context, options) {
      return call("actions.run", {
        id: id,
        context: context || null,
        ai: (options && options.ai) || null,
      });
    },
    onRun: function (runId, fn) {
      return this.on("run", function (event) {
        if (event && event.runId === runId) fn(event);
      });
    },
    settings: {
      get: function () {
        return call("settings.get");
      },
      set: function (patch) {
        return call("settings.set", { patch: patch });
      },
    },
    secrets: {
      set: function (id, value) {
        return call("secrets.set", { id: id, value: value });
      },
      has: function (id) {
        return call("secrets.has", { id: id });
      },
    },
    storage: {
      get: function (key) {
        return call("storage.get", { key: key });
      },
      set: function (key, value) {
        return call("storage.set", { key: key, value: value });
      },
    },
  };
})(window);
