/**
 * Browser Git helpers for git-core's web backend.
 *
 * Loads isomorphic-git from esm.sh (or a local node_modules path if present)
 * and stores repos under OPFS. Exposes `globalThis.GitWeb` for wasm-bindgen.
 */
import {
  joinPath,
  normalizeRel,
  pathParts,
  makeStats,
  statusLabel,
} from "./git-web-helpers.js";

const DEFAULT_CORS = "https://cors.isomorphic-git.org";

let gitMod = null;
let httpMod = null;

async function loadGit() {
  if (gitMod && httpMod) return { git: gitMod, http: httpMod };
  const candidates = [
    "./node_modules/isomorphic-git/index.js",
    "/node_modules/isomorphic-git/index.js",
    "https://esm.sh/isomorphic-git@1.30.1",
  ];
  const httpCandidates = [
    "./node_modules/isomorphic-git/http/web/index.js",
    "/node_modules/isomorphic-git/http/web/index.js",
    "https://esm.sh/isomorphic-git@1.30.1/http/web",
  ];
  let lastErr;
  for (let i = 0; i < candidates.length; i++) {
    try {
      gitMod = (await import(candidates[i])).default || (await import(candidates[i]));
      httpMod = (await import(httpCandidates[i])).default || (await import(httpCandidates[i]));
      return { git: gitMod, http: httpMod };
    } catch (e) {
      lastErr = e;
    }
  }
  throw lastErr || new Error("failed to load isomorphic-git");
}

async function getRoot() {
  return navigator.storage.getDirectory();
}

async function resolveDir(root, path, { create } = { create: false }) {
  const parts = pathParts(path);
  let dir = root;
  for (const part of parts) {
    dir = await dir.getDirectoryHandle(part, { create });
  }
  return dir;
}

async function resolveParent(root, filePath, { create } = { create: false }) {
  const parts = pathParts(filePath);
  const name = parts.pop();
  if (!name) {
    const err = new Error("ENOENT: empty path");
    err.code = "ENOENT";
    throw err;
  }
  let dir = root;
  for (const part of parts) {
    dir = await dir.getDirectoryHandle(part, { create });
  }
  return { dir, name };
}

/** Minimal promise-based fs for isomorphic-git, backed by OPFS. */
function createOpfsFs() {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  return {
    promises: {
      async readFile(path, options) {
        const root = await getRoot();
        const { dir, name } = await resolveParent(root, path, { create: false });
        const handle = await dir.getFileHandle(name);
        const file = await handle.getFile();
        const buf = new Uint8Array(await file.arrayBuffer());
        if (options && (options.encoding === "utf8" || options === "utf8")) {
          return decoder.decode(buf);
        }
        return buf;
      },
      async writeFile(path, data, _options) {
        const root = await getRoot();
        const { dir, name } = await resolveParent(root, path, { create: true });
        const handle = await dir.getFileHandle(name, { create: true });
        const writable = await handle.createWritable();
        const bytes =
          typeof data === "string"
            ? encoder.encode(data)
            : data instanceof Uint8Array
              ? data
              : new Uint8Array(data);
        await writable.write(bytes);
        await writable.close();
      },
      async unlink(path) {
        const root = await getRoot();
        const { dir, name } = await resolveParent(root, path, { create: false });
        await dir.removeEntry(name);
      },
      async readdir(path) {
        const root = await getRoot();
        const dir = await resolveDir(root, path, { create: false });
        const names = [];
        for await (const [name] of dir.entries()) names.push(name);
        return names;
      },
      async mkdir(path, _options) {
        const root = await getRoot();
        // No-op for '.' / root — isomorphic-git mkdir parents can request these.
        if (pathParts(path).length === 0) return;
        await resolveDir(root, path, { create: true });
      },
      async rmdir(path) {
        const root = await getRoot();
        const parts = pathParts(path);
        const name = parts.pop();
        if (!name) {
          const err = new Error("ENOENT: cannot rmdir root");
          err.code = "ENOENT";
          throw err;
        }
        let dir = root;
        for (const part of parts) dir = await dir.getDirectoryHandle(part);
        await dir.removeEntry(name, { recursive: false });
      },
      async stat(path) {
        const root = await getRoot();
        try {
          const parts = pathParts(path);
          // `lstat('.')` / `lstat('$workdir/.')` → directory stats for that dir.
          if (parts.length === 0) {
            return makeStats({
              isFile: false,
              size: 0,
              mtimeMs: Date.now(),
              mode: 0o040755,
            });
          }
          let dir = root;
          for (let i = 0; i < parts.length - 1; i++) {
            dir = await dir.getDirectoryHandle(parts[i]);
          }
          const name = parts[parts.length - 1];
          try {
            const fh = await dir.getFileHandle(name);
            const file = await fh.getFile();
            return makeStats({
              isFile: true,
              size: file.size,
              mtimeMs: file.lastModified || Date.now(),
            });
          } catch (_) {
            await dir.getDirectoryHandle(name);
            return makeStats({
              isFile: false,
              size: 0,
              mtimeMs: Date.now(),
              mode: 0o040755,
            });
          }
        } catch (e) {
          const err = new Error(e && e.message ? e.message : String(e));
          err.code = "ENOENT";
          throw err;
        }
      },
      async lstat(path) {
        return this.stat(path);
      },
      async readlink(_path) {
        const err = new Error("EINVAL");
        err.code = "EINVAL";
        throw err;
      },
      async symlink(_target, _path) {
        const err = new Error("EPERM");
        err.code = "EPERM";
        throw err;
      },
      async chmod(_path, _mode) {},
    },
  };
}

const fs = createOpfsFs();

async function ensureDir(path) {
  const root = await getRoot();
  await resolveDir(root, path, { create: true });
}

async function init(workdir) {
  const { git } = await loadGit();
  await ensureDir(workdir);
  await git.init({ fs, dir: workdir });
  return null;
}

async function clone(url, workdir, corsProxy) {
  const { git, http } = await loadGit();
  await ensureDir(workdir);
  const opts = {
    fs,
    http,
    dir: workdir,
    url,
    singleBranch: true,
    depth: 1,
  };
  const proxy = corsProxy && String(corsProxy).trim();
  if (proxy) opts.corsProxy = proxy;
  else opts.corsProxy = DEFAULT_CORS;
  await git.clone(opts);
  return null;
}

async function list(workdir, rel) {
  const root = await getRoot();
  const path = joinPath(workdir, normalizeRel(rel));
  const dir = await resolveDir(root, path, { create: false });
  const out = [];
  for await (const [name, handle] of dir.entries()) {
    if (name === ".git") continue;
    out.push({ path: name, is_dir: handle.kind === "directory" });
  }
  out.sort((a, b) => a.path.localeCompare(b.path));
  return out;
}

async function readFile(workdir, rel) {
  const path = joinPath(workdir, normalizeRel(rel));
  const data = await fs.promises.readFile(path);
  return data instanceof Uint8Array ? data : new Uint8Array(data);
}

async function writeFile(workdir, rel, data) {
  const { git } = await loadGit();
  const path = joinPath(workdir, normalizeRel(rel));
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data);
  await fs.promises.writeFile(path, bytes);
  await git.add({ fs, dir: workdir, filepath: normalizeRel(rel) });
  return null;
}

async function removeFile(workdir, rel) {
  const { git } = await loadGit();
  const filepath = normalizeRel(rel);
  try {
    await git.remove({ fs, dir: workdir, filepath });
  } catch (_) {
    const path = joinPath(workdir, filepath);
    try {
      await fs.promises.unlink(path);
    } catch (e) {
      /* ignore missing */
    }
  }
  return null;
}

async function status(workdir) {
  const { git } = await loadGit();
  const matrix = await git.statusMatrix({ fs, dir: workdir });
  const out = [];
  for (const [filepath, head, workdirStat, stage] of matrix) {
    const label = statusLabel(head, workdirStat, stage);
    if (label !== "clean") out.push({ path: filepath, status: label });
  }
  return out;
}

globalThis.GitWeb = {
  init,
  clone,
  list,
  readFile,
  writeFile,
  removeFile,
  status,
};
