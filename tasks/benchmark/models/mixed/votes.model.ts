import { m } from "@earnest/model";

const Phase = m.enum(["Idle", "Voting", "Done"]);

export const Votes = m.model("Votes", {
	phase: Phase.initial("Idle"),
	yes: m.int(0, 3).initial(0),
	no: m.int(0, 3).initial(0),
});

export const open = Votes.action("the chair opens the vote", {
	when: (s) => s.phase === "Idle",
	then: () => ({ phase: "Voting" }),
});

export const voteYes = Votes.action("a member votes yes", {
	when: (s) => s.phase === "Voting" && s.yes + s.no < 3,
	then: (s) => ({ yes: s.yes + 1 }),
});

export const voteNo = Votes.action("a member votes no", {
	when: (s) => s.phase === "Voting" && s.yes + s.no < 3,
	then: (s) => ({ no: s.no + 1 }),
});

export const close = Votes.action("the chair closes the vote", {
	when: (s) => s.phase === "Voting",
	then: () => ({ phase: "Done" }),
});

export const few = Votes.rule({
	says: "At most three votes are cast",
	holds: (s) => s.yes + s.no <= 3,
});

export const decided = Votes.end({
	says: "The vote is closed",
	when: (s) => s.phase === "Done",
});
