import { m } from "@earnest/model";

export const Upload = m.model("Upload", {
	call: m.request(),
	backoff: m.timer(),
	tries: m.retry({ max: 2 }),
});

export const send = Upload.action("the app sends the file", {
	when: (s) => s.call.isIdle && s.backoff.isIdle,
	then: (s) => ({ call: s.call.send() }),
});

export const succeed = Upload.action("the server accepts the file", {
	when: (s) => s.call.isPending,
	then: (s) => ({ call: s.call.succeed() }),
});

export const fail = Upload.action("the server rejects the file", {
	when: (s) => s.call.isPending,
	then: (s) => ({ call: s.call.fail() }),
});

export const wait = Upload.action("the app backs off before another try", {
	when: (s) => s.call.hasFailed && s.tries.canRetry && s.backoff.isIdle,
	then: (s) => ({ call: s.call.reset(), backoff: s.backoff.start(), tries: s.tries.again() }),
});

export const wake = Upload.action("the backoff timer fires", {
	when: (s) => s.backoff.isRunning,
	then: (s) => ({ backoff: s.backoff.fire() }),
});

export const rearm = Upload.action("the app clears the fired timer", {
	when: (s) => s.backoff.hasFired,
	then: (s) => ({ backoff: s.backoff.reset() }),
});

export const abort = Upload.action("the user starts over while the app waits", {
	when: (s) => s.backoff.isRunning,
	then: (s) => ({ backoff: s.backoff.cancel(), tries: s.tries.reset() }),
});

export const quiet = Upload.rule({
	says: "A settled call has no timer running",
	holds: (s) => !s.call.isSettled || s.backoff.isIdle,
});

export const waiting = Upload.rule({
	says: "While the timer is not idle the call is idle",
	holds: (s) => s.backoff.isIdle || s.call.isIdle,
});

export const counted = Upload.rule({
	says: "The tries never pass the maximum",
	holds: (s) => s.tries.attempts <= 2 && (!s.tries.isExhausted || !s.tries.canRetry),
});

export const sent = Upload.end({
	says: "The file is uploaded",
	when: (s) => s.call.hasSucceeded,
});

export const spent = Upload.end({
	says: "Every try failed",
	when: (s) => s.call.hasFailed && s.tries.isExhausted,
});

export type State = m.Infer<typeof Upload>;
