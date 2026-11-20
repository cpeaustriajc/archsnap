import { type ChildProcess, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import {
  type Machine,
  SCOPE,
  SUPPORTED,
  binaryFile,
  currentMachine,
  isSupported,
  packageName,
  platformKey,
} from "./platform.ts";

export const BINARY_VARIABLE = "EARNEST_BINARY";
export const NO_BINARY_EXIT = 127;

const FORWARDED = ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT", "SIGBREAK"] as const;

export type Resolve = (id: string) => string;

export class NoBinary extends Error {}

const SUPPORTED_LIST = SUPPORTED.join(", ");

export const locate = (machine: Machine, env: NodeJS.ProcessEnv, resolve: Resolve): string => {
  const given = env[BINARY_VARIABLE];
  if (given !== undefined && given !== "") {
    return given;
  }
  const key = platformKey(machine);
  if (!isSupported(key)) {
    throw new NoBinary(
      `earnest has no binary for ${key}, and ${SCOPE}/cli ships one only for ${SUPPORTED_LIST}.`,
    );
  }
  const name = packageName(key);
  let manifest: string;
  try {
    manifest = resolve(`${name}/package.json`);
  } catch {
    throw new NoBinary(
      `earnest has no binary for ${key} because ${name} is not installed, so install ${SCOPE}/cli again with optional dependencies enabled.`,
    );
  }
  const binary = join(dirname(manifest), "bin", binaryFile(machine.platform));
  if (!existsSync(binary)) {
    throw new NoBinary(
      `earnest has no binary for ${key} because ${name} holds no ${binaryFile(machine.platform)}, so install it again.`,
    );
  }
  return binary;
};

const forwardSignals = (child: ChildProcess): (() => void) => {
  const handlers = FORWARDED.map((signal) => {
    const handler = () => {
      child.kill(signal);
    };
    process.on(signal, handler);
    return [signal, handler] as const;
  });
  return () => {
    for (const [signal, handler] of handlers) {
      process.off(signal, handler);
    }
  };
};

const fail = (message: string): never => {
  process.stderr.write(`${message}\n`);
  return process.exit(NO_BINARY_EXIT);
};

export const run = (binary: string, args: readonly string[]): void => {
  const child = spawn(binary, args, { stdio: "inherit", windowsHide: true });
  const stopForwarding = forwardSignals(child);
  child.on("error", (error) => {
    stopForwarding();
    fail(`earnest could not start ${binary}: ${error.message}.`);
  });
  child.on("close", (code, signal) => {
    stopForwarding();
    if (signal !== null) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
};

export const main = (args: readonly string[]): void => {
  const require = createRequire(import.meta.url);
  try {
    run(
      locate(currentMachine(), process.env, (id) => require.resolve(id)),
      args,
    );
  } catch (error) {
    if (error instanceof NoBinary) {
      fail(error.message);
      return;
    }
    throw error;
  }
};
