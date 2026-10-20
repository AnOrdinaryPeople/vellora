/**
 * CommonJS consumers (issue #38): a project compiled with `"module": "commonjs"` emits
 * `require("vellora")`. The packages ship one ESM build; the `default` export condition lets Node's
 * `require(esm)` (Node 20.19+ / 22.12+) resolve it instead of failing with
 * `ERR_PACKAGE_PATH_NOT_EXPORTED`. A single build keeps module state (the active native bridge)
 * shared between `import` and `require` callers.
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

const PACKAGES_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const LIBRARY_PACKAGES = ["vellora", "native", "lint", "engine-chromium"];

type ExportsMap = Record<string, Record<string, string>>;

function readExports(dir: string): ExportsMap {
  const manifest = JSON.parse(readFileSync(join(PACKAGES_DIR, dir, "package.json"), "utf8"));
  return manifest.exports as ExportsMap;
}

describe("CommonJS entry point", () => {
  test.each(LIBRARY_PACKAGES)("%s exports a require-resolvable root entry", (dir) => {
    const root = readExports(dir)["."];
    expect(Object.keys(root ?? {})[0]).toBe("types");
    expect(root?.default).toBe(root?.import);
  });

  const built = LIBRARY_PACKAGES.every((dir) =>
    existsSync(join(PACKAGES_DIR, dir, "dist", "index.js")),
  );
  const [major = 0, minor = 0] = process.versions.node.split(".").map(Number);
  const requireEsm = major > 22 || (major === 22 && minor >= 12) || (major === 20 && minor >= 19);

  test.skipIf(!built || !requireEsm)("require() loads the built packages", () => {
    const script = [
      'const vellora = require("vellora");',
      'if (typeof vellora.renderPdf !== "function") throw new Error("renderPdf missing");',
      'for (const name of ["@vellora/native", "@vellora/lint", "@vellora/engine-chromium"]) require(name);',
    ].join("\n");
    const result = spawnSync(process.execPath, ["--input-type=commonjs", "-e", script], {
      cwd: PACKAGES_DIR,
      encoding: "utf8",
    });
    expect(result.status, result.stderr).toBe(0);
  });
});
