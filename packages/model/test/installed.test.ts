import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, expect, test } from "vitest";

const PACKAGE = resolve(import.meta.dirname, "..");
const TYPE_CHECKER = join(PACKAGE, "node_modules", ".bin", "tsrs");
const SLOW = 120_000;
const NOWHERE = "http://127.0.0.1:9";

const offline: NodeJS.ProcessEnv = {
  ...process.env,
  NPM_CONFIG_REGISTRY: NOWHERE,
  npm_config_registry: NOWHERE,
};

const MODEL = `import { m } from "@earnest/model";

export const Counter = m.model("Counter", { count: m.int(0, 2).initial(0) });

export const increment = Counter.action("the count goes up", {
  when: (s) => s.count < 2,
  then: (s) => ({ count: s.count + 1 }),
});

export const full = Counter.end({ says: "The count is full", when: (s) => s.count === 2 });

export type State = m.Infer<typeof Counter>;
`;

const RUN = `import { explore, m, replay } from "@earnest/model";
import * as model from "./counter.model.ts";

const found = explore(model.Counter, { exports: model });
const passed = replay(
  model.Counter,
  {
    start: [{ name: "count", value: 0 }],
    steps: [{ action: "increment", state: [{ name: "count", value: 1 }] }],
  },
  { exports: model },
);
console.log(JSON.stringify({ states: found.states, status: found.status, replayed: passed.length, m: typeof m.int }));
`;

const STRICT = {
  compilerOptions: {
    target: "esnext",
    module: "nodenext",
    lib: ["esnext"],
    types: [],
    strict: true,
    noEmit: true,
    noUncheckedIndexedAccess: true,
    exactOptionalPropertyTypes: true,
    verbatimModuleSyntax: true,
    erasableSyntaxOnly: true,
    allowImportingTsExtensions: true,
  },
  include: ["*.model.ts"],
};

let scratch = "";
let project = "";

const typeCheck = () =>
  spawnSync(TYPE_CHECKER, ["-p", join(project, "tsconfig.json")], { encoding: "utf8" });

beforeAll(() => {
  scratch = mkdtempSync(join(tmpdir(), "earnest-model-installed-"));
  const packed = join(scratch, "packed");
  project = join(scratch, "project");
  mkdirSync(packed);
  mkdirSync(project);
  execFileSync("bun", ["pm", "pack", "--destination", packed, "--quiet"], {
    cwd: PACKAGE,
    stdio: "pipe",
  });
  const [tarball] = readdirSync(packed);
  writeFileSync(
    join(project, "package.json"),
    JSON.stringify({ name: "consumer", private: true, type: "module" }),
  );
  writeFileSync(join(project, "tsconfig.json"), JSON.stringify(STRICT));
  writeFileSync(join(project, "counter.model.ts"), MODEL);
  writeFileSync(join(project, "run.ts"), RUN);
  execFileSync("bun", ["add", join(packed, tarball ?? "")], {
    cwd: project,
    stdio: "pipe",
    env: offline,
  });
}, SLOW);

afterAll(() => {
  rmSync(scratch, { recursive: true, force: true });
});

test("the installed package runs a model's explore under Node", () => {
  const output = execFileSync("node", ["run.ts"], { cwd: project, encoding: "utf8" });
  expect(JSON.parse(output)).toEqual({ states: 3, status: "pass", replayed: 2, m: "function" });
});

test("the installed package type-checks a model file under strict settings", () => {
  const result = typeCheck();
  expect(result.stdout + result.stderr).toBe("");
  expect(result.status).toBe(0);
});

test("the installed package still rejects a wrong model at type-check time", () => {
  writeFileSync(
    join(project, "wrong.model.ts"),
    `import { m } from "@earnest/model";\nconst Wrong = m.model("Wrong", { n: m.int(0, 1).initial(0) });\nexport const bad = Wrong.action("bad", { then: () => ({ n: "x" }) });\n`,
  );
  const result = typeCheck();
  rmSync(join(project, "wrong.model.ts"));
  expect(result.status).not.toBe(0);
  expect(result.stdout + result.stderr).toContain("wrong.model.ts");
});
