/**
 * @vellora/native — platform loader.
 *
 * Resolves the correct prebuilt napi-rs `.node` addon for the host (a locally built addon in dev,
 * or a per-platform `optionalDependencies` package once published), and throws a clear, actionable
 * error when no compatible addon is found. Re-exports the addon's async, thread-safe `render`
 * function (libuv-threadpool, per-call isolated) unchanged.
 */
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
// Both `src/` (vitest) and `dist/` (built) live one level under the package root, where the
// locally built `.node` is emitted, so `..` resolves to the package dir in both cases.
const PKG_DIR = join(dirname(fileURLToPath(import.meta.url)), "..");

/** The only platforms this phase ships prebuilds for. */
export const SUPPORTED_PLATFORMS = [
  "macOS arm64 (darwin-arm64)",
  "macOS x64 (darwin-x64)",
  "linux x64 glibc (linux-x64-gnu)",
  "linux arm64 glibc (linux-arm64-gnu)",
] as const;

/**
 * Host-tag → prebuild package map. Pre-enumerates the full intended matrix so adding a target later
 * is purely additive (publish the package, flip its value from `null`, add its literal `require` in
 * `requirePrebuild`): no loader-shape or consumer change. Published in the launch matrix: `darwin-arm64`, `darwin-x64`, `linux-x64-gnu`,
 * `linux-arm64-gnu`. Reserved (no published package yet): `linux-x64-musl`, `linux-arm64-musl`.
 */
export const RESOLUTION_TABLE: Readonly<Record<string, string | null>> = {
  "darwin-arm64": "@vellora/native-darwin-arm64",
  "darwin-x64": "@vellora/native-darwin-x64",
  "linux-x64-gnu": "@vellora/native-linux-x64-gnu",
  "linux-arm64-gnu": "@vellora/native-linux-arm64-gnu",
  // Reserved for a later phase: no published package yet (musl needs a native-musl build host).
  "linux-x64-musl": null,
  "linux-arm64-musl": null,
};

/**
 * Render options forwarded to `vellora-core`. Producer is fixed to `vellora`; the document title, a
 * deterministic creation date `[year, month, day]` (never wall-clock), and optional image assets are
 * caller-supplied.
 */
export interface RenderOpts {
  title?: string;
  creationDate?: [number, number, number];
  /** Image bytes keyed by an `<img>`'s `src` string; the format is detected in the core. */
  images?: Record<string, Uint8Array>;
  /** Base URL used only to normalize a relative `<img>` `src` into the `images` lookup key. */
  baseUrl?: string;
  /** Custom font faces (raw TTF/OTF bytes); family/weight/style are read from the bytes in the core. */
  fonts?: Uint8Array[];
  /** Emit an archival PDF using the named PDF/A profile. Currently only PDF/A-2b is supported. */
  pdfa?: "PDF/A-2b";
}

/** Surface of the native addon: the smoke export plus the async `render` binding. */
export interface VelloraAddon {
  coreName(): string;
  render(html: Uint8Array, opts?: RenderOpts): Promise<Uint8Array>;
  /** Test-only: forces a worker-thread panic to verify panic-to-rejection. Not a public API. */
  __forcePanicForTest?(): Promise<void>;
}

function isMusl(): boolean {
  try {
    const report = process.report?.getReport() as
      | { header?: { glibcVersionRuntime?: string } }
      | undefined;
    return !report?.header?.glibcVersionRuntime;
  } catch {
    return false;
  }
}

/** napi-rs platform tag, e.g. `darwin-arm64`, `linux-x64-gnu`, `linux-arm64-musl`. */
export function platformTag(
  platform: string = process.platform,
  arch: string = process.arch,
): string {
  if (platform === "linux") {
    return `linux-${arch}-${isMusl() ? "musl" : "gnu"}`;
  }
  return `${platform}-${arch}`;
}

/** Build the actionable error thrown when no compatible addon exists for the host. */
export function unsupportedPlatformError(
  platform: string,
  arch: string,
  tag: string = platformTag(platform, arch),
): Error {
  const windowsNote =
    platform === "win32"
      ? "\nWindows is not supported yet — prebuilt Windows binaries are a planned fast-follow."
      : "";
  return new Error(
    `@vellora/native: no compatible prebuilt addon for ${platform}-${arch} (resolved tag "${tag}").
Supported platforms: ${SUPPORTED_PLATFORMS.join(", ")}.${windowsNote}
To build locally, install the Rust toolchain and run \`npm run build\` at the repo root (which runs \`napi build\` for the vellora-napi crate).`,
  );
}

/**
 * Require the published prebuild package for `tag`. Each specifier is a string literal (never
 * computed from `RESOLUTION_TABLE`) so bundlers and file tracers can resolve the `.node` statically.
 * Every non-null `RESOLUTION_TABLE` entry must have a matching case here.
 */
function requirePrebuild(tag: string): VelloraAddon | undefined {
  switch (tag) {
    case "darwin-arm64":
      return require("@vellora/native-darwin-arm64") as VelloraAddon;
    case "darwin-x64":
      return require("@vellora/native-darwin-x64") as VelloraAddon;
    case "linux-x64-gnu":
      return require("@vellora/native-linux-x64-gnu") as VelloraAddon;
    case "linux-arm64-gnu":
      return require("@vellora/native-linux-arm64-gnu") as VelloraAddon;
    default:
      return undefined;
  }
}

function loadAddon(): VelloraAddon {
  const tag = platformTag();
  // 1) Dev: a locally built `.node` next to the package takes precedence.
  const localPath = join(PKG_DIR, `vellora.${tag}.node`);
  if (existsSync(localPath)) {
    return require(localPath) as VelloraAddon;
  }
  // 2) Published: require the matching prebuild package. Only the host-matching entry is
  // required; a missing sibling optional package must not break the load.
  try {
    const prebuild = requirePrebuild(tag);
    if (prebuild) return prebuild;
  } catch {
    // The optional prebuild package is absent on this host; fall through to the actionable error.
  }
  throw unsupportedPlatformError(process.platform, process.arch, tag);
}

let cached: VelloraAddon | undefined;

/** Resolve and cache the host addon. Throws an actionable error on an unsupported platform. */
export function addon(): VelloraAddon {
  if (!cached) {
    cached = loadAddon();
  }
  return cached;
}

/** Smoke export: proves the addon loads and is callable in-process. */
export function coreName(): string {
  return addon().coreName();
}

/**
 * Render document HTML *content* bytes to a PDF on the libuv threadpool, off the Node main thread.
 * `html` is always content (never a file path); the returned `Uint8Array` is JS-owned and starts
 * with `%PDF-`. Rejects with an `Error` carrying the `vellora-core` diagnostic — for a located
 * diagnostic the error also exposes `{ feature, line, col, hint }` as machine-readable properties.
 */
export function render(html: Uint8Array, opts?: RenderOpts): Promise<Uint8Array> {
  return addon().render(html, opts);
}
