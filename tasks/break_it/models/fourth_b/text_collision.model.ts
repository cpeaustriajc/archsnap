import { m } from "@earnest/model";

const Letter = m.enum(["Z", "Ś"]);

export const Pick = m.model("Pick", { letter: Letter.initial("Z") });

export const choose = m.action(Pick, "a letter is chosen", {
	input: { v: Letter },
	changes: (_, { v }) => ({ letter: v }),
});

export const never = m.end(Pick, "Never", () => false);

export const any = m.rule(Pick, "Any letter will do", () => true);
