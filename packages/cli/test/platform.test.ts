import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { expect, test } from "vitest";
import { BINARY_VARIABLE, NoBinary, locate } from "../src/launch.ts";
import { SUPPORTED, packageName, platformKey } from "../src/platform.ts";

const PACKAGE = resolve(import.meta.dirname, "..");
const PLATFORMS = join(PACKAGE, "platforms");

const manifest = (file: string): Record<string, unknown> =>
  JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;

const absent = (id: string): string => {
  throw new Error(`cannot find ${id}`);
};

test("a platform key names the operating system, the cpu and, where it matters, the libc", () => {
  expect(platformKey({ platform: "linux", arch: "x64", libc: "gnu" })).toBe("linux-x64-gnu");
  expect(platformKey({ platform: "linux", arch: "arm64", libc: "musl" })).toBe("linux-arm64-musl");
  expect(platformKey({ platform: "darwin", arch: "arm64", libc: null })).toBe("darwin-arm64");
  expect(platformKey({ platform: "win32", arch: "x64", libc: null })).toBe("win32-x64-msvc");
});

test("every supported platform has a package folder with matching name, os, cpu and libc", () => {
  expect(readdirSync(PLATFORMS).sort()).toEqual([...SUPPORTED].sort());
  for (const key of SUPPORTED) {
    const fields = manifest(join(PLATFORMS, key, "package.json"));
    const [platform, arch, abi] = key.split("-");
    expect(fields.name).toBe(packageName(key));
    expect(fields.os).toEqual([platform]);
    expect(fields.cpu).toEqual([arch]);
    expect(fields.libc).toEqual(
      platform === "linux" ? [abi === "musl" ? "musl" : "glibc"] : undefined,
    );
    expect(fields.scripts).toBeUndefined();
  }
});

test("the launcher package lists every platform package at its own version, and installs nothing at install time", () => {
  const launcher = manifest(join(PACKAGE, "package.json"));
  const dependencies = launcher.optionalDependencies as Record<string, string>;
  expect(Object.keys(dependencies).sort()).toEqual(SUPPORTED.map(packageName).sort());
  for (const key of SUPPORTED) {
    const fields = manifest(join(PLATFORMS, key, "package.json"));
    expect(fields.version).toBe(launcher.version);
    expect(dependencies[packageName(key)]).toBe(`workspace:${String(launcher.version)}`);
  }
  const scripts = Object.keys(launcher.scripts as Record<string, string>);
  expect(scripts.filter((name) => /^(pre|post)?install$/.test(name))).toEqual([]);
  expect(launcher.dependencies).toBeUndefined();
});

test("a platform with no package gets one sentence that names the platform", () => {
  const error = (() => {
    try {
      locate({ platform: "linux", arch: "x64", libc: "musl" }, {}, absent);
    } catch (caught) {
      return caught;
    }
    return null;
  })();
  expect(error).toBeInstanceOf(NoBinary);
  const message = (error as NoBinary).message;
  expect(message).toContain("linux-x64-musl");
  expect(message).not.toContain("\n");
  expect(message.match(/\.\s/g)).toBeNull();
  expect(message.endsWith(".")).toBe(true);
});

test("a supported platform whose package is not installed gets one sentence that names it", () => {
  expect(() => locate({ platform: "darwin", arch: "arm64", libc: null }, {}, absent)).toThrow(
    /darwin-arm64 because @earnest\/cli-darwin-arm64 is not installed/,
  );
});

test("the binary is found next to the package.json of the platform package", () => {
  const root = mkdtempSync(join(tmpdir(), "earnest-locate-"));
  try {
    mkdirSync(join(root, "bin"));
    writeFileSync(join(root, "bin", "earnest"), "");
    const resolve = (id: string): string => {
      expect(id).toBe("@earnest/cli-linux-x64-gnu/package.json");
      return join(root, "package.json");
    };
    expect(locate({ platform: "linux", arch: "x64", libc: "gnu" }, {}, resolve)).toBe(
      join(root, "bin", "earnest"),
    );
    expect(() => locate({ platform: "win32", arch: "x64", libc: null }, {}, resolve)).toThrow(
      NoBinary,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the binary variable overrides the platform package", () => {
  expect(
    locate({ platform: "plan9", arch: "mips", libc: null }, { [BINARY_VARIABLE]: "/x" }, absent),
  ).toBe("/x");
});
