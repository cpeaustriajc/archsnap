declare module "node:test" {
	export function test(name: string, run: () => void | Promise<void>): void;
}

declare module "node:assert/strict" {
	interface Assert {
		equal(actual: unknown, expected: unknown, message?: string): void;
		notEqual(actual: unknown, expected: unknown, message?: string): void;
		deepEqual(actual: unknown, expected: unknown, message?: string): void;
		ok(value: unknown, message?: string): asserts value;
		throws(run: () => unknown, expected: RegExp, message?: string): void;
	}
	const assert: Assert;
	export default assert;
}

declare module "node:child_process" {
	export function spawnSync(
		command: string,
		parameters: readonly string[],
		options: { readonly cwd: string; readonly encoding: "utf8" },
	): { readonly status: number | null; readonly stdout: string; readonly stderr: string };
}

declare module "node:fs" {
	export function readFileSync(path: string, encoding: "utf8"): string;
	export function readdirSync(path: string): string[];
}

interface ImportMeta {
	readonly dirname: string;
}
