import { m } from "@earnest/model";

export const Unsafe = m.model("Unsafe", {
	n: m.int(9007199254740991, 9007199254740992),
});

export const sane = m.rule(Unsafe, "One more than the number is more", (s) => s.n + 1 > s.n);

export const stop = m.end(Unsafe, "Any state may stop", () => true);
