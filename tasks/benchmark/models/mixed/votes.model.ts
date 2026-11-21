import { m } from "@earnest/model";

const Phase = m.enum(["Idle", "Voting", "Done"]);

export const Votes = m.model("Votes", {
	phase: Phase.initial("Idle"),
	yes: m.int(0, 3).initial(0),
	no: m.int(0, 3).initial(0),
});

export const open = m.action(Votes, "the chair opens the vote", {
	when: (s) => s.phase === "Idle",
	changes: () => ({ phase: "Voting" }),
});

export const voteYes = m.action(Votes, "a member votes yes", {
	when: (s) => s.phase === "Voting" && s.yes + s.no < 3,
	changes: (s) => ({ yes: s.yes + 1 }),
});

export const voteNo = m.action(Votes, "a member votes no", {
	when: (s) => s.phase === "Voting" && s.yes + s.no < 3,
	changes: (s) => ({ no: s.no + 1 }),
});

export const close = m.action(Votes, "the chair closes the vote", {
	when: (s) => s.phase === "Voting",
	changes: () => ({ phase: "Done" }),
});

export const few = m.rule(Votes, "At most three votes are cast", (s) => s.yes + s.no <= 3);

export const decided = m.end(Votes, "The vote is closed", (s) => s.phase === "Done");
