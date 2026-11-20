import { type ChildProcess, spawn } from "node:child_process";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, expect, test } from "vitest";
import { BINARY_VARIABLE, NO_BINARY_EXIT } from "../src/launch.ts";

const LAUNCHER = resolve(import.meta.dirname, "../src/earnest.ts");

const FAKE = `#!/usr/bin/env node
const [mode, ...rest] = process.argv.slice(2);
if (mode === "echo") {
  let input = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => (input += chunk));
  process.stdin.on("end", () => {
    process.stdout.write(JSON.stringify({ args: process.argv.slice(2), input }) + "\\n");
    process.stderr.write("to stderr\\n");
    process.exit(Number(process.env.FAKE_EXIT ?? 0));
  });
} else if (mode === "exit") {
  process.exit(Number(rest[0]));
} else if (mode === "wait") {
  process.on("SIGTERM", () => {
    process.stdout.write("child got SIGTERM\\n");
    process.exit(42);
  });
  process.stdout.write("ready\\n");
  setInterval(() => {}, 1000);
} else if (mode === "die") {
  process.kill(process.pid, rest[0]);
  setInterval(() => {}, 1000);
}
`;

interface Outcome {
  readonly code: number | null;
  readonly signal: NodeJS.Signals | null;
  readonly stdout: string;
  readonly stderr: string;
}

let scratch = "";
let fake = "";

beforeAll(() => {
  scratch = mkdtempSync(join(tmpdir(), "earnest-launcher-"));
  fake = join(scratch, "fake-earnest");
  writeFileSync(fake, FAKE);
  chmodSync(fake, 0o755);
});

afterAll(() => {
  rmSync(scratch, { recursive: true, force: true });
});

const launch = (args: readonly string[], env: NodeJS.ProcessEnv = {}): ChildProcess =>
  spawn("node", [LAUNCHER, ...args], {
    env: { ...process.env, [BINARY_VARIABLE]: fake, ...env },
    stdio: ["pipe", "pipe", "pipe"],
  });

const finish = (child: ChildProcess): Promise<Outcome> =>
  new Promise((done) => {
    let stdout = "";
    let stderr = "";
    child.stdout?.on("data", (chunk: Buffer) => (stdout += chunk.toString()));
    child.stderr?.on("data", (chunk: Buffer) => (stderr += chunk.toString()));
    child.on("close", (code, signal) => {
      done({ code, signal, stdout, stderr });
    });
  });

test("arguments, stdin, stdout and stderr pass through unchanged", async () => {
  const child = launch(["echo", "verify", "--json", "", "two words", "--max-states=5"]);
  child.stdin?.end("typed input");
  const outcome = await finish(child);
  expect(outcome.code).toBe(0);
  expect(JSON.parse(outcome.stdout)).toEqual({
    args: ["echo", "verify", "--json", "", "two words", "--max-states=5"],
    input: "typed input",
  });
  expect(outcome.stderr).toBe("to stderr\n");
});

test.each([0, 1, 2, 37])(
  "exit code %i of the binary is the exit code of the launcher",
  async (code) => {
    const outcome = await finish(launch(["exit", String(code)]));
    expect(outcome.code).toBe(code);
    expect(outcome.signal).toBeNull();
  },
);

test("a signal sent to the launcher reaches the binary, and its exit code comes back", async () => {
  const child = launch(["wait"]);
  await new Promise<void>((ready) => {
    child.stdout?.once("data", () => {
      ready();
    });
  });
  child.kill("SIGTERM");
  const outcome = await finish(child);
  expect(outcome.stdout).toContain("child got SIGTERM");
  expect(outcome.code).toBe(42);
});

test("a binary killed by a signal makes the launcher die of the same signal", async () => {
  const outcome = await finish(launch(["die", "SIGTERM"]));
  expect(outcome.signal).toBe("SIGTERM");
  expect(outcome.code).toBeNull();
});

test("a missing binary is one sentence on stderr and its own exit code", async () => {
  const outcome = await finish(launch(["verify"], { [BINARY_VARIABLE]: join(scratch, "absent") }));
  expect(outcome.code).toBe(NO_BINARY_EXIT);
  expect(outcome.stdout).toBe("");
  expect(outcome.stderr.trimEnd().split("\n")).toHaveLength(1);
  expect(outcome.stderr).toContain("absent");
});
