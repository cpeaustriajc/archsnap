import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

const PACKAGE = `${import.meta.dirname}/..`;
const FOLDER = "test/rejected";

const REJECTED_AT: Readonly<Record<string, string>> = {
	"input-compared-to-stranger": `by === "touch"`,
	"missing-then": `Cart.action(`,
	"null-into-required": `cta: null`,
	"optional-read-unguarded": `s.pressBy.length`,
	"rule-not-boolean": `holds: (s) => s.count,`,
	"state-assigned": `s.count = 2`,
	"stranger-in-set": `s.seen.add("touch")`,
	"unknown-color": `stuck:`,
	"unknown-field": `basket: 1`,
	"unknown-field-in-one-branch": `then: (s) => {`,
	"unknown-input": `{ who }`,
	"unknown-state-field": `s.basket`,
	"unknown-view-key": `colours:`,
	"wrong-initial": `.initial("medium")`,
	"wrong-kind": `count: "one"`,
	"wrong-literal": `cta: "Nope"`,
};

const ACCEPTED = ["accepted"];

interface Problem {
	readonly program: string;
	readonly line: string;
}

const compiled = spawnSync("tsc", ["--noEmit", "--pretty", "false", "-p", FOLDER], {
	cwd: PACKAGE,
	encoding: "utf8",
});

const problems: Problem[] = compiled.stdout
	.split("\n")
	.map((reported) => /^test\/rejected\/([\w-]+)\.ts\((\d+),\d+\): error TS\d+/.exec(reported))
	.flatMap((match) => {
		const [, program, lineNumber] = match ?? [];
		if (program === undefined || lineNumber === undefined) {
			return [];
		}
		const lines = readFileSync(`${PACKAGE}/${FOLDER}/${program}.ts`, "utf8").split("\n");
		return [{ program, line: lines[Number(lineNumber) - 1] ?? "" }];
	});

const problemsIn = (program: string): string[] =>
	problems.filter((problem) => problem.program === program).map((problem) => problem.line.trim());

test("every program in the folder is expected to be accepted or rejected", () => {
	const programs = readdirSync(`${PACKAGE}/${FOLDER}`)
		.filter((file) => file.endsWith(".ts"))
		.map((file) => file.slice(0, -".ts".length))
		.sort();
	assert.deepEqual(programs, [...ACCEPTED, ...Object.keys(REJECTED_AT)].sort());
});

test("the compiler ran and found problems", () => {
	assert.equal(compiled.status, 2, compiled.stderr);
	assert.ok(problems.length > 0, compiled.stdout);
});

for (const program of ACCEPTED) {
	test(`the compiler accepts ${program}`, () => {
		assert.deepEqual(problemsIn(program), []);
	});
}

for (const [program, place] of Object.entries(REJECTED_AT)) {
	test(`the compiler rejects ${program} at \`${place}\``, () => {
		const [line, ...others] = problemsIn(program);
		assert.ok(line !== undefined, `no problem was reported in ${program}`);
		assert.deepEqual(others, []);
		assert.ok(line.includes(place), `the problem is at \`${line}\`, not at \`${place}\``);
	});
}
