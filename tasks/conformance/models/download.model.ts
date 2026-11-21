import { m } from "@earnest/model";

export const Download = m.model("Download", {
	phase: m.enum(["Idle", "Loading", "Done", "Failed"]).initial("Idle"),
	saved: m.boolean().initial(false),
});

export const start = m.action(Download, "the user starts the download", {
	when: (s) => s.phase === "Idle",
	changes: () => ({ phase: "Loading" }),
});

export const finish = m.action(Download, "the last byte arrives", {
	when: (s) => s.phase === "Loading",
	changes: () => ({ phase: "Done", saved: true }),
});

export const fail = m.action(Download, "the connection drops", {
	when: (s) => s.phase === "Loading",
	changes: () => ({ phase: "Failed" }),
});

export const whole = m.rule(Download, "Only a finished download is saved", (s) => !s.saved || s.phase === "Done");

export const done = m.end(Download, "The file is on disk", (s) => s.phase === "Done");

export type State = m.Infer<typeof Download>;
