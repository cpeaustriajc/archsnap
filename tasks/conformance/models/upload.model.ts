import { m } from "@earnest/model";

export const Upload = m.model("Upload", {
	call: m.request(),
	backoff: m.timer(),
	tries: m.retry({ max: 2 }),
});

export const send = m.action(Upload, "the app sends the file", {
	when: (s) => s.call.isIdle && s.backoff.isIdle,
	changes: (s) => ({ call: s.call.send() }),
});

export const succeed = m.action(Upload, "the server accepts the file", {
	when: (s) => s.call.isPending,
	changes: (s) => ({ call: s.call.succeed() }),
});

export const fail = m.action(Upload, "the server rejects the file", {
	when: (s) => s.call.isPending,
	changes: (s) => ({ call: s.call.fail() }),
});

export const wait = m.action(Upload, "the app backs off before another try", {
	when: (s) => s.call.hasFailed && s.tries.canRetry && s.backoff.isIdle,
	changes: (s) => ({ call: s.call.reset(), backoff: s.backoff.start(), tries: s.tries.again() }),
});

export const wake = m.action(Upload, "the backoff timer fires", {
	when: (s) => s.backoff.isRunning,
	changes: (s) => ({ backoff: s.backoff.fire() }),
});

export const rearm = m.action(Upload, "the app clears the fired timer", {
	when: (s) => s.backoff.hasFired,
	changes: (s) => ({ backoff: s.backoff.reset() }),
});

export const abort = m.action(Upload, "the user starts over while the app waits", {
	when: (s) => s.backoff.isRunning,
	changes: (s) => ({ backoff: s.backoff.cancel(), tries: s.tries.reset() }),
});

export const quiet = m.rule(
	Upload,
	"A settled call has no timer running",
	(s) => !s.call.isSettled || s.backoff.isIdle,
);

export const waiting = m.rule(
	Upload,
	"While the timer is not idle the call is idle",
	(s) => s.backoff.isIdle || s.call.isIdle,
);

export const counted = m.rule(
	Upload,
	"The tries never pass the maximum",
	(s) => s.tries.attempts <= 2 && (!s.tries.isExhausted || !s.tries.canRetry),
);

export const sent = m.end(Upload, "The file is uploaded", (s) => s.call.hasSucceeded);

export const spent = m.end(Upload, "Every try failed", (s) => s.call.hasFailed && s.tries.isExhausted);

export type State = m.Infer<typeof Upload>;
