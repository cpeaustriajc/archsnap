import { m } from "@earnest/model";

export const Unsafe = m.model("Unsafe", {
	n: m.int(9007199254740991, 9007199254740992),
});

export const sane = Unsafe.rule({
	says: "One more than the number is more",
	holds: (s) => s.n + 1 > s.n,
});

export const stop = Unsafe.end({
	says: "Any state may stop",
	when: () => true,
});
