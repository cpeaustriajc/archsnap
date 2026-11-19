import { m } from "@earnest/model";

const Worker = m.enum(["Ada", "Bo"]);

export const Printer = m.model("Printer", {
	tray: m.lock(),
	job: m.lock(Worker),
});

export const claim = Printer.action("a worker claims the printer", {
	input: { worker: Worker },
	when: (s) => s.job.isFree,
	then: (s, { worker }) => ({ job: s.job.acquire(worker) }),
});

export const load = Printer.action("the worker loads the tray", {
	when: (s) => s.job.isHeld && s.tray.isFree,
	then: (s) => ({ tray: s.tray.acquire() }),
});

export const print = Printer.action("the page prints", {
	when: (s) => s.tray.isHeld,
	then: (s) => ({ tray: s.tray.release(), job: s.job.release() }),
});

export const guarded = Printer.rule({
	says: "The tray is only held while a worker holds the printer",
	holds: (s) => !s.tray.isHeld || s.job.isHeld,
});

export const owned = Printer.rule({
	says: "The holder is the worker who holds the printer",
	holds: (s) =>
		m.forall(Worker, (worker) => !s.job.isHeldBy(worker) || s.job.holder === worker) &&
		(s.job.isHeld || s.job.holder === null),
});

export type State = m.Infer<typeof Printer>;
