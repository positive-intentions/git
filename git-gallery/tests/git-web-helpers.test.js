import { describe, expect, it } from "vitest";
import {
  joinPath,
  normalizeRel,
  pathParts,
  makeStats,
  statusLabel,
  authCallback,
  unifiedDiff,
  guessLanguage,
} from "../assets/git-web-helpers.js";

describe("joinPath", () => {
  it("joins segments and strips extra slashes", () => {
    expect(joinPath("/repos/a", "notes/x.txt")).toBe("/repos/a/notes/x.txt");
    expect(joinPath("/repos/a/", "/notes")).toBe("/repos/a/notes");
  });

  it("returns left path for empty or dot right", () => {
    expect(joinPath("/repos/a/", ".")).toBe("/repos/a");
    expect(joinPath("/repos/a", "")).toBe("/repos/a");
    expect(joinPath("/", ".")).toBe("/");
  });
});

describe("normalizeRel", () => {
  it("strips leading slashes and accepts nested paths", () => {
    expect(normalizeRel("/foo")).toBe("foo");
    expect(normalizeRel("notes/hello.txt")).toBe("notes/hello.txt");
    expect(normalizeRel(".")).toBe("");
    expect(normalizeRel("")).toBe("");
  });

  it("rejects ..", () => {
    expect(() => normalizeRel("a/../b")).toThrow(/path must not contain '\.\.'/);
  });
});

describe("pathParts", () => {
  it("collapses empty and dot segments", () => {
    expect(pathParts("/a/./b/")).toEqual(["a", "b"]);
    expect(pathParts(".")).toEqual([]);
    expect(pathParts("")).toEqual([]);
  });

  it("rejects .. with EINVAL", () => {
    try {
      pathParts("a/../b");
      expect.unreachable();
    } catch (e) {
      expect(e.code).toBe("EINVAL");
    }
  });
});

describe("makeStats", () => {
  it("builds file stats with defaults", () => {
    const s = makeStats({ isFile: true, size: 3, mtimeMs: 1_700_000_000_000 });
    expect(s.isFile()).toBe(true);
    expect(s.isDirectory()).toBe(false);
    expect(s.size).toBe(3);
    expect(s.mode).toBe(0o100644);
    expect(s.mtimeMs).toBe(1_700_000_000_000);
    expect(s.ctime).toBeInstanceOf(Date);
  });

  it("builds directory stats", () => {
    const s = makeStats({ isFile: false, size: 0, mtimeMs: NaN });
    expect(s.isDirectory()).toBe(true);
    expect(s.mode).toBe(0o040755);
    expect(Number.isFinite(s.mtimeMs)).toBe(true);
  });
});

describe("statusLabel", () => {
  it.each([
    [0, 2, 2, "staged"],
    [0, 2, 0, "untracked"],
    [1, 2, 2, "staged"],
    [1, 2, 1, "modified"],
    [1, 0, 1, "deleted"],
    [1, 0, 0, "deleted"],
    [1, 1, 1, "clean"],
    [1, 1, 2, "changed"],
  ])("maps (%i, %i, %i) -> %s", (head, workdir, stage, expected) => {
    expect(statusLabel(head, workdir, stage)).toBe(expected);
  });
});

describe("authCallback", () => {
  it("returns null when incomplete", () => {
    expect(authCallback("", "t")).toBeNull();
    expect(authCallback("u", "")).toBeNull();
    expect(authCallback("  ", "t")).toBeNull();
  });

  it("returns onAuth that yields username/password", () => {
    const cb = authCallback("user", "token");
    expect(cb()).toEqual({ username: "user", password: "token" });
  });
});

describe("unifiedDiff", () => {
  it("returns empty for identical inputs", () => {
    expect(unifiedDiff("a.txt", "x", "x")).toBe("");
  });

  it("emits a simple unified hunk", () => {
    const d = unifiedDiff("a.txt", "one\ntwo", "one\nthree");
    expect(d).toContain("diff --git a/a.txt b/a.txt");
    expect(d).toContain("-two");
    expect(d).toContain("+three");
    expect(d).toContain(" one");
  });
});

describe("guessLanguage", () => {
  it.each([
    ["src/main.rs", "rust"],
    ["a.js", "javascript"],
    ["a.ts", "typescript"],
    ["a.json", "json"],
    ["README.md", "markdown"],
    ["x.unknown", "plaintext"],
    ["noext", "plaintext"],
  ])("%s -> %s", (path, lang) => {
    expect(guessLanguage(path)).toBe(lang);
  });
});
