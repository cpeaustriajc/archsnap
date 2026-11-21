import { m } from "@earnest/model";

export const Wide = m.model("Wide", {
	x: m.int(0, 99).initial(0),
	y: m.int(0, 99).initial(0),
});

export const spin = m.action(Wide, "the wheels spin by three amounts", {
	input: { a: m.int(0, 9), b: m.int(0, 9), c: m.int(0, 9) },
	changes: (s, { a, b, c }) => ({ x: m.rem(s.x + a, 100), y: m.rem(s.y + b + c, 100) }),
});

export const dials = m.rule(Wide, "Both wheels stay on their dials", (s) => s.x <= 99 && s.y <= 99);

export type State = m.Infer<typeof Wide>;
