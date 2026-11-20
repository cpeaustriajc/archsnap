import { m } from "@earnest/model";

const Letter = m.enum(["Z", "Ś"]);

export const Pick = m.model("Pick", { letter: Letter.initial("Z") });

export const choose = Pick.action("a letter is chosen", {
	input: { v: Letter },
	then: (_, { v }) => ({ letter: v }),
});

export const never = Pick.end({ says: "Never", when: () => false });

export const any = Pick.rule({ says: "Any letter will do", holds: () => true });
