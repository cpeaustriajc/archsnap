import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, expect, test } from "vitest";
import { NO_BINARY_EXIT } from "../src/launch.ts";
import { binaryFile, currentMachine, packageName, platformKey } from "../src/platform.ts";

const PACKAGE = resolve(import.meta.dirname, "..");
const REPOSITORY = resolve(PACKAGE, "../..");
const SLOW = 900_000;
const NOWHERE = "http://127.0.0.1:9";

const PASSING = `import { m } from "@earnest/model";

export const Counter = m.model("Counter", { count: m.int(0, 2).initial(0) });

export const increment = m.action(Counter, "the count goes up", {
  when: (s) => s.count < 2,
  changes: (s) => ({ count: s.count + 1 }),
});

export const bounded = m.rule(Counter, "The count stays small", (s) => s.count <= 2);

export const full = m.end(Counter, "The count is full", (s) => s.count === 2);
`;

const FAILING = `import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
  gate: m.enum(["Locked", "Open"]).initial("Locked"),
  paid: m.boolean().initial(false),
});

export const pay = m.action(Turnstile, "the visitor pays", {
  when: (s) => !s.paid && s.gate === "Locked",
  changes: () => ({ paid: true }),
});

export const push = m.action(Turnstile, "the visitor pushes", {
  when: (s) => s.gate === "Locked",
  changes: () => ({ gate: "Open" }),
});

export const enter = m.action(Turnstile, "the visitor walks through", {
  when: (s) => s.gate === "Open",
  changes: () => ({ gate: "Locked", paid: false }),
});

export const locked = m.rule(
  Turnstile,
  "The gate stays locked until the visitor pays",
  (s) => s.gate === "Locked" || s.paid,
);
`;

interface Outcome {
  readonly code: number | null;
  readonly stdout: string;
  readonly stderr: string;
}

let scratch = "";
let project = "";
let packed = "";

const offline: NodeJS.ProcessEnv = {
  ...process.env,
  NPM_CONFIG_REGISTRY: NOWHERE,
  npm_config_registry: NOWHERE,
};

const bunPack = (folder: string, destination: string): void => {
  execFileSync("bun", ["pm", "pack", "--destination", destination, "--quiet"], {
    cwd: folder,
    stdio: "pipe",
  });
};

const bunAdd = (folder: string, tarballs: string[]): void => {
  execFileSync("bun", ["add", ...tarballs], { cwd: folder, stdio: "pipe", env: offline });
};

const targetFolder = (): string => process.env.CARGO_TARGET_DIR ?? join(REPOSITORY, "target");

const earnest = (...args: string[]): Outcome => {
  const result = spawnSync(join(project, "node_modules", ".bin", "earnest"), args, {
    cwd: project,
    encoding: "utf8",
  });
  return { code: result.status, stdout: result.stdout, stderr: result.stderr };
};

const manifestInside = (tarball: string): Record<string, unknown> =>
  JSON.parse(
    execFileSync("tar", ["--extract", "--to-stdout", "--file", tarball, "package/package.json"], {
      encoding: "utf8",
    }),
  ) as Record<string, unknown>;

beforeAll(() => {
  const machine = currentMachine();
  scratch = mkdtempSync(join(tmpdir(), "earnest-cli-installed-"));
  packed = join(scratch, "packed");
  project = join(scratch, "project");
  const platform = join(scratch, "platform");
  mkdirSync(packed);
  mkdirSync(join(project, "models"), { recursive: true });

  execFileSync("cargo", ["build", "--release", "--locked", "-p", "earnest-cli"], {
    cwd: REPOSITORY,
    stdio: "pipe",
  });
  const binary = join(targetFolder(), "release", binaryFile(machine.platform));
  expect(existsSync(binary)).toBe(true);

  execFileSync(
    "node",
    [join(PACKAGE, "scripts", "assemble.ts"), "--binary", binary, "--out", platform],
    { cwd: PACKAGE, stdio: "pipe" },
  );
  bunPack(platform, packed);
  bunPack(PACKAGE, packed);

  writeFileSync(
    join(project, "package.json"),
    JSON.stringify({ name: "consumer", private: true, type: "module" }),
  );
  writeFileSync(join(project, "models", "passing.model.ts"), PASSING);
  writeFileSync(join(project, "models", "failing.model.ts"), FAILING);
  const tarballs = readdirSync(packed).map((name) => join(packed, name));
  bunAdd(project, tarballs);
}, SLOW);

afterAll(() => {
  rmSync(scratch, { recursive: true, force: true });
});

test("the two tarballs hold the launcher, the binary and no install script", () => {
  const key = platformKey(currentMachine());
  const tarballs = readdirSync(packed).sort();
  expect(tarballs).toEqual([`earnest-cli-${key}-0.1.0.tgz`, "earnest-cli-0.1.0.tgz"].sort());
  const launcher = manifestInside(join(packed, "earnest-cli-0.1.0.tgz"));
  const platform = manifestInside(join(packed, `earnest-cli-${key}-0.1.0.tgz`));
  expect(platform.name).toBe(packageName(key));
  const dependencies = launcher.optionalDependencies as Record<string, string>;
  expect(Object.values(dependencies).every((range) => range === launcher.version)).toBe(true);
  for (const fields of [launcher, platform]) {
    const scripts = Object.keys((fields.scripts ?? {}) as Record<string, string>);
    expect(scripts.filter((name) => /install$/.test(name))).toEqual([]);
  }
  const files = (tarball: string): string[] =>
    execFileSync("tar", ["--list", "--file", join(packed, tarball)], { encoding: "utf8" })
      .split("\n")
      .filter((line) => line !== "");
  expect(files(`earnest-cli-${key}-0.1.0.tgz`)).toContain(
    `package/bin/${binaryFile(currentMachine().platform)}`,
  );
  expect(files("earnest-cli-0.1.0.tgz")).toContain("package/dist/earnest.js");
});

test("verify passes a passing model and exits 0", () => {
  const outcome = earnest("verify", "models/passing.model.ts");
  expect(outcome.stderr).toBe("");
  expect(outcome.stdout).toMatch(/^pass\s+Counter\s+3 states\s+\d+ ms$/m);
  expect(outcome.stdout).toContain("1 model: 1 pass, 0 fail, 0 unfinished");
  expect(outcome.code).toBe(0);
});

test("verify reports a broken rule with its path and exits 1", () => {
  const outcome = earnest("verify", "models/failing.model.ts");
  expect(outcome.stdout).toMatch(/^fail\s+Turnstile\s+4 states/m);
  expect(outcome.stdout).toContain(
    'broken  locked  "The gate stays locked until the visitor pays"',
  );
  expect(outcome.stdout).toContain('1. push  gate: "Locked" -> "Open"');
  expect(outcome.stdout).toContain("1 model: 0 pass, 1 fail, 0 unfinished");
  expect(outcome.code).toBe(1);
});

test("a bad flag exits 2 with the checker's own message", () => {
  const outcome = earnest("verify", "--bogus");
  expect(outcome.stdout).toBe("");
  expect(outcome.stderr).toContain("unexpected argument '--bogus'");
  expect(outcome.code).toBe(2);
});

test("a project that holds only the launcher gets one sentence naming the platform", () => {
  const bare = join(scratch, "bare");
  mkdirSync(bare);
  writeFileSync(join(bare, "package.json"), JSON.stringify({ name: "bare", private: true }));
  const launcher = join(packed, "earnest-cli-0.1.0.tgz");
  bunAdd(bare, [launcher]);
  const result = spawnSync(join(bare, "node_modules", ".bin", "earnest"), ["verify"], {
    cwd: bare,
    encoding: "utf8",
  });
  const key = platformKey(currentMachine());
  expect(result.status).toBe(NO_BINARY_EXIT);
  expect(result.stdout).toBe("");
  expect(result.stderr.trimEnd().split("\n")).toHaveLength(1);
  expect(result.stderr).toContain(key);
  expect(result.stderr).toContain(packageName(key));
});

test("json output passes through unchanged", () => {
  const outcome = earnest("verify", "--json", "--no-cache", "models/passing.model.ts");
  const result = JSON.parse(outcome.stdout) as { summary: { pass: number }; models: unknown[] };
  expect(result.summary.pass).toBe(1);
  expect(result.models).toHaveLength(1);
  expect(outcome.code).toBe(0);
});

test("the installed launcher is the package's bin", () => {
  const launcher = readFileSync(
    join(project, "node_modules", "@earnest", "cli", "dist", "earnest.js"),
    "utf8",
  );
  expect(launcher.startsWith("#!/usr/bin/env node")).toBe(true);
  expect(existsSync(join(project, "node_modules", ".bin", "earnest"))).toBe(true);
});
