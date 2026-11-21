import { m } from "@earnest/model";

const Worker = m.enum(["Ada", "Bo"]);

export const Printer = m.model("Printer", {
	tray: m.lock(),
	job: m.lock(Worker),
});

export const claim = m.action(Printer, "a worker claims the printer", {
	input: { worker: Worker },
	when: (s) => s.job.isFree,
	changes: (s, { worker }) => ({ job: s.job.acquire(worker) }),
});

export const load = m.action(Printer, "the worker loads the tray", {
	when: (s) => s.job.isHeld && s.tray.isFree,
	changes: (s) => ({ tray: s.tray.acquire() }),
});

export const print = m.action(Printer, "the page prints", {
	when: (s) => s.tray.isHeld,
	changes: (s) => ({ tray: s.tray.release(), job: s.job.release() }),
});

export const guarded = m.rule(
	Printer,
	"The tray is only held while a worker holds the printer",
	(s) => !s.tray.isHeld || s.job.isHeld,
);

export const owned = m.rule(
	Printer,
	"The holder is the worker who holds the printer",
	(s) =>
		m.forall(Worker, (worker) => !s.job.isHeldBy(worker) || s.job.holder === worker) &&
		(s.job.isHeld || s.job.holder === null),
);

export type State = m.Infer<typeof Printer>;
