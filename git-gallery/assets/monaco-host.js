/**
 * Monaco editor host for the git-gallery Clone story (web only).
 * Loads Monaco from jsDelivr; exposes `globalThis.MonacoHost`.
 *
 * Dioxus may replace the host DOM node on re-render; callers should use
 * `ensureMounted` / async `setValue` so the editor tracks the live element.
 *
 * Wrapped in an IIFE so classic-script double injection (hashed + plain asset)
 * does not throw on redeclared top-level const.
 */
(function () {
  if (globalThis.MonacoHost) return;

  const MONACO_BASE =
    "https://cdn.jsdelivr.net/npm/monaco-editor@0.52.2/min/vs";

  const DEFAULT_DOM_ID = "git-gallery-monaco";
  /** Fixed pixel height — never rely on flex/grid computed size (often collapses). */
  const EDITOR_HEIGHT_PX = 288;

  let editor = null;
  let monacoApi = null;
  let loadPromise = null;
  let mountedDomId = null;

  function loadScript(src) {
    return new Promise((resolve, reject) => {
      const existing = document.querySelector(`script[src="${src}"]`);
      if (existing) {
        if (existing.dataset.loaded === "1") {
          resolve();
          return;
        }
        existing.addEventListener("load", () => resolve(), { once: true });
        return;
      }
      const s = document.createElement("script");
      s.src = src;
      s.async = true;
      s.onload = () => {
        s.dataset.loaded = "1";
        resolve();
      };
      s.onerror = () => reject(new Error(`failed to load ${src}`));
      document.head.appendChild(s);
    });
  }

  async function ensureMonaco() {
    if (monacoApi) return monacoApi;
    if (loadPromise) return loadPromise;
    loadPromise = (async () => {
      await loadScript(`${MONACO_BASE}/loader.js`);
      const requireFn = globalThis.require;
      if (!requireFn || !requireFn.config) {
        throw new Error("Monaco AMD loader missing");
      }
      requireFn.config({ paths: { vs: MONACO_BASE } });
      monacoApi = await new Promise((resolve, reject) => {
        try {
          requireFn(
            ["vs/editor/editor.main"],
            () => {
              resolve(globalThis.monaco);
            },
            reject
          );
        } catch (e) {
          reject(e);
        }
      });
      return monacoApi;
    })();
    return loadPromise;
  }

  /** Always pin an editable box — gallery Tailwind has no height utilities. */
  function forceHostBox(el) {
    el.style.boxSizing = "border-box";
    el.style.display = "block";
    el.style.width = "100%";
    el.style.height = `${EDITOR_HEIGHT_PX}px`;
    el.style.minHeight = `${EDITOR_HEIGHT_PX}px`;
    el.style.maxHeight = `${EDITOR_HEIGHT_PX}px`;
    el.style.position = "relative";
    el.style.overflow = "hidden";
    el.style.flex = "none";
  }

  function editorStillLive(domId) {
    if (!editor) return false;
    let node = null;
    try {
      node = editor.getContainerDomNode();
    } catch (_) {
      return false;
    }
    if (!node || !node.isConnected) return false;
    const host = document.getElementById(domId);
    if (!host) return false;
    return host === node || host.contains(node);
  }

  async function mount(domId) {
    const id = domId || DEFAULT_DOM_ID;
    const monaco = await ensureMonaco();
    const el = document.getElementById(id);
    if (!el) throw new Error(`Monaco mount target #${id} not found`);
    forceHostBox(el);
    if (editor) {
      try {
        editor.dispose();
      } catch (_) {
        /* ignore */
      }
      editor = null;
    }
    el.innerHTML = "";
    editor = monaco.editor.create(el, {
      value: "",
      language: "plaintext",
      automaticLayout: true,
      readOnly: false,
      minimap: { enabled: false },
      fontSize: 13,
      theme: "vs",
      scrollBeyondLastLine: false,
    });
    mountedDomId = id;
    forceHostBox(el);
    const monacoRoot = el.querySelector(".monaco-editor");
    if (monacoRoot) {
      monacoRoot.style.width = "100%";
      monacoRoot.style.height = "100%";
    }
    // Double rAF: wait for layout after Dioxus paint.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!editor) return;
        forceHostBox(el);
        editor.layout({ width: el.clientWidth, height: EDITOR_HEIGHT_PX });
        editor.focus();
      });
    });
    return null;
  }

  async function ensureMounted(domId) {
    const id = domId || DEFAULT_DOM_ID;
    if (editorStillLive(id)) {
      const el = document.getElementById(id);
      if (el) {
        forceHostBox(el);
        editor.layout({ width: el.clientWidth, height: EDITOR_HEIGHT_PX });
      }
      return null;
    }
    await mount(id);
    return null;
  }

  async function setValue(text, language, domId) {
    const id = domId || mountedDomId || DEFAULT_DOM_ID;
    await ensureMounted(id);
    if (!editor || !monacoApi) return null;
    const model = editor.getModel();
    const lang = language || "plaintext";
    if (model) {
      monacoApi.editor.setModelLanguage(model, lang);
      editor.setValue(text == null ? "" : String(text));
    }
    const el = document.getElementById(id);
    if (el) forceHostBox(el);
    editor.layout({
      width: el ? el.clientWidth : undefined,
      height: EDITOR_HEIGHT_PX,
    });
    editor.updateOptions({ readOnly: false });
    return null;
  }

  async function getValue(domId) {
    await ensureMounted(domId || mountedDomId || DEFAULT_DOM_ID);
    if (!editor) return "";
    return editor.getValue();
  }

  async function setLanguage(language, domId) {
    await ensureMounted(domId || mountedDomId || DEFAULT_DOM_ID);
    if (!editor || !monacoApi) return null;
    const model = editor.getModel();
    if (model) monacoApi.editor.setModelLanguage(model, language || "plaintext");
    return null;
  }

  function dispose() {
    if (editor) {
      try {
        editor.dispose();
      } catch (_) {
        /* ignore */
      }
      editor = null;
    }
    mountedDomId = null;
    return null;
  }

  const STORAGE_KEY = "git-gallery:clone";

  function storageLoad() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      return JSON.parse(raw);
    } catch (_) {
      return null;
    }
  }

  function storageSave(data) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch (_) {
      /* quota / private mode */
    }
    return null;
  }

  globalThis.MonacoHost = {
    mount,
    ensureMounted,
    setValue,
    getValue,
    setLanguage,
    dispose,
    storageLoad,
    storageSave,
  };
})();
