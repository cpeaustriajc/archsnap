import { m } from "@earnest/model";

export const Wide = m.model("Wide", {
	x: m.int(0, 99).initial(0),
	y: m.int(0, 99).initial(0),
});

export const spin = Wide.action("the wheels spin by three amounts", {
	input: { a: m.int(0, 9), b: m.int(0, 9), c: m.int(0, 9) },
	then: (s, { a, b, c }) => ({ x: m.rem(s.x + a, 100), y: m.rem(s.y + b + c, 100) }),
});

export const dials = Wide.rule({
	says: "Both wheels stay on their dials",
	holds: (s) => s.x <= 99 && s.y <= 99,
});

export type State = m.Infer<typeof Wide>;
