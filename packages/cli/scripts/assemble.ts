import { chmodSync, copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import {
  type Machine,
  SUPPORTED,
  binaryFile,
  currentMachine,
  isSupported,
  packageName,
  platformKey,
} from "../src/platform.ts";

const CLI_PACKAGE = resolve(import.meta.dirname, "..");
const REPOSITORY = resolve(CLI_PACKAGE, "../..");
const EXECUTABLE = 0o755;

const USAGE = `usage: node scripts/assemble.ts --out <folder> [--binary <path>]

Writes the package for this machine's platform into <folder>: the built earnest binary
(default: target/release/earnest) under bin/, and the package.json kept in platforms/ with its bin entry added.
Pack it with \`bun pm pack\` in that folder.`;

export interface Assembled {
  readonly name: string;
  readonly folder: string;
}

export const assemble = (machine: Machine, binary: string, out: string): Assembled => {
  const key = platformKey(machine);
  if (!isSupported(key)) {
    throw new Error(
      `no platform package is listed for ${key}; @earnest/cli ships ${SUPPORTED.join(", ")}`,
    );
  }
  const name = packageName(key);
  const folder = resolve(out);
  rmSync(folder, { recursive: true, force: true });
  mkdirSync(join(folder, "bin"), { recursive: true });
  const target = join(folder, "bin", binaryFile(machine.platform));
  copyFileSync(binary, target);
  chmodSync(target, EXECUTABLE);
  const stub = JSON.parse(
    readFileSync(join(CLI_PACKAGE, "platforms", key, "package.json"), "utf8"),
  );
  const manifest = { ...stub, bin: { [`earnest-${key}`]: `bin/${binaryFile(machine.platform)}` } };
  writeFileSync(join(folder, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  return { name, folder };
};

const main = (): void => {
  const { values } = parseArgs({
    options: {
      out: { type: "string" },
      binary: { type: "string" },
      help: { type: "boolean" },
    },
  });
  if (values.help === true || values.out === undefined) {
    process.stderr.write(`${USAGE}\n`);
    process.exit(values.help === true ? 0 : 2);
  }
  const binary = resolve(
    values.binary ?? join(REPOSITORY, "target", "release", binaryFile(process.platform)),
  );
  const { name, folder } = assemble(currentMachine(), binary, values.out);
  process.stdout.write(`${name} assembled in ${folder}\n`);
};

if (process.argv[1] !== undefined && dirname(resolve(process.argv[1])) === import.meta.dirname) {
  main();
}
