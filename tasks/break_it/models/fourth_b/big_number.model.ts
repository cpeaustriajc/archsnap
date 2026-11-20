import { m } from "@earnest/model";

export const Big = m.model("Big", { n: m.int(2147483645, 2147483650).initial(2147483645) });

export const inc = Big.action("one up", { when: (s) => s.n < 2147483650, then: (s) => ({ n: s.n + 1 }) });

export const top = Big.end({ says: "At the top", when: (s) => s.n === 2147483650 });

export const low = Big.rule({ says: "Below the top", holds: (s) => s.n < 2147483649 });
export const prod = Big.rule({ says: "Product", holds: (s) => s.n * 2 > 0 });
