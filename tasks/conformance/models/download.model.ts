import { m } from "@earnest/model";

export const Download = m.model("Download", {
	phase: m.enum(["Idle", "Loading", "Done", "Failed"]).initial("Idle"),
	saved: m.boolean().initial(false),
});

export const start = Download.action("the user starts the download", {
	when: (s) => s.phase === "Idle",
	then: () => ({ phase: "Loading" }),
});

export const finish = Download.action("the last byte arrives", {
	when: (s) => s.phase === "Loading",
	then: () => ({ phase: "Done", saved: true }),
});

export const fail = Download.action("the connection drops", {
	when: (s) => s.phase === "Loading",
	then: () => ({ phase: "Failed" }),
});

export const whole = Download.rule({
	says: "Only a finished download is saved",
	holds: (s) => !s.saved || s.phase === "Done",
});

export const done = Download.end({
	says: "The file is on disk",
	when: (s) => s.phase === "Done",
});

export type State = m.Infer<typeof Download>;
