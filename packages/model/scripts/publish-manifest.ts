import { copyFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const PACKAGE = resolve(import.meta.dirname, "..");
const MANIFEST = join(PACKAGE, "package.json");
const SOURCE = join(PACKAGE, "node_modules", ".package.source.json");

const restore = (): void => {
  if (existsSync(SOURCE)) {
    copyFileSync(SOURCE, MANIFEST);
    rmSync(SOURCE);
  }
};

const apply = (): void => {
  restore();
  copyFileSync(MANIFEST, SOURCE);
  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    publishConfig: { exports: unknown };
  };
  const published = { ...manifest, exports: manifest.publishConfig.exports };
  writeFileSync(MANIFEST, `${JSON.stringify(published, null, 2)}\n`);
};

const [command] = process.argv.slice(2);
if (command === "apply") {
  apply();
} else if (command === "restore") {
  restore();
} else {
  process.stderr.write("usage: node scripts/publish-manifest.ts apply|restore\n");
  process.exit(2);
}
