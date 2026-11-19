import { readdirSync } from "node:fs";

for (const file of readdirSync(import.meta.dirname).sort()) {
	if (file.endsWith(".test.ts")) {
		await import(`./${file}`);
	}
}
