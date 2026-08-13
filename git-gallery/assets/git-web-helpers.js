/**
 * Pure helpers shared by git-web.js and unit tests.
 */

export function joinPath(a, b) {
  if (!b || b === "." || b === "/") return a.replace(/\/+$/, "") || "/";
  const left = a.replace(/\/+$/, "");
  const right = String(b).replace(/^\/+/, "");
  return `${left}/${right}`;
}

export function normalizeRel(rel) {
  const s = String(rel || "")
    .replace(/^\/+/, "")
    .replace(/\\/g, "/");
  if (s.split("/").includes("..")) throw new Error("path must not contain '..'");
  return s === "." ? "" : s;
}

/**
 * Collapse '.', reject '..', drop empty segments.
 * isomorphic-git walks from filepath '.' and then lstats `$dir/.`.
 */
export function pathParts(path) {
  const parts = String(path || "")
    .replace(/\\/g, "/")
    .split("/")
    .filter((p) => p && p !== ".");
  if (parts.includes("..")) {
    const err = new Error("EINVAL: path must not contain '..'");
    err.code = "EINVAL";
    throw err;
  }
  return parts;
}

/**
 * Node-like fs.Stats shape for isomorphic-git.
 * Missing ctime/mtime causes: Cannot read properties of undefined (reading 'valueOf').
 */
export function makeStats({ isFile, size, mtimeMs, mode }) {
  const ms = Number.isFinite(mtimeMs) ? mtimeMs : Date.now();
  const date = new Date(ms);
  const fileMode = mode != null ? mode : isFile ? 0o100644 : 0o040755;
  return {
    isFile: () => !!isFile,
    isDirectory: () => !isFile,
    isSymbolicLink: () => false,
    size: size || 0,
    mode: fileMode,
    mtimeMs: ms,
    ctimeMs: ms,
    mtime: date,
    ctime: date,
    atimeMs: ms,
    atime: date,
    birthtimeMs: ms,
    birthtime: date,
    uid: 0,
    gid: 0,
    dev: 0,
    ino: 0,
    nlink: 1,
  };
}

/**
 * Map isomorphic-git statusMatrix row [HEAD, WORKDIR, STAGE] to a short label.
 * Returns "clean" when there is nothing to surface.
 */
export function statusLabel(head, workdirStat, stage) {
  if (head === 0 && workdirStat === 2 && stage === 2) return "staged";
  if (head === 0 && workdirStat === 2 && stage === 0) return "untracked";
  if (head === 1 && workdirStat === 2 && stage === 2) return "staged";
  if (head === 1 && workdirStat === 2 && stage === 1) return "modified";
  if (head === 1 && workdirStat === 0) return "deleted";
  if (workdirStat !== head || stage !== head) return "changed";
  return "clean";
}

/**
 * isomorphic-git `onAuth` callback when username+token are both set.
 * Returns null when auth should be omitted.
 */
export function authCallback(username, token) {
  const u = String(username || "").trim();
  const t = String(token || "").trim();
  if (!u || !t) return null;
  return () => ({ username: u, password: t });
}

/**
 * Minimal unified diff for gallery debugging (not a full Myers diff).
 */
export function unifiedDiff(path, before, after) {
  if (before === after) return "";

  const a = String(before ?? "").split("\n");
  const b = String(after ?? "").split("\n");
  const lines = [
    `diff --git a/${path} b/${path}`,
    `--- a/${path}`,
    `+++ b/${path}`,
  ];
  const max = Math.max(a.length, b.length);
  // Emit a single hunk covering the whole file (good enough for demos).
  lines.push(`@@ -1,${a.length || 0} +1,${b.length || 0} @@`);
  for (let i = 0; i < max; i++) {
    const left = i < a.length ? a[i] : undefined;
    const right = i < b.length ? b[i] : undefined;
    if (left === right) {
      if (left !== undefined) lines.push(` ${left}`);
    } else {
      if (left !== undefined) lines.push(`-${left}`);
      if (right !== undefined) lines.push(`+${right}`);
    }
  }
  return lines.join("\n");
}

/** Monaco / highlight language id from file extension. */
export function guessLanguage(path) {
  const name = String(path || "").toLowerCase();
  const ext = name.includes(".") ? name.split(".").pop() : "";
  switch (ext) {
    case "rs":
      return "rust";
    case "js":
    case "mjs":
    case "cjs":
      return "javascript";
    case "ts":
    case "tsx":
      return "typescript";
    case "json":
      return "json";
    case "md":
      return "markdown";
    case "toml":
      return "ini";
    case "yml":
    case "yaml":
      return "yaml";
    case "css":
      return "css";
    case "html":
    case "htm":
      return "html";
    case "py":
      return "python";
    case "sh":
      return "shell";
    default:
      return "plaintext";
  }
}
