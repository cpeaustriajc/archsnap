import { m } from "@earnest/model";

export const Big = m.model("Big", { n: m.int(2147483645, 2147483650).initial(2147483645) });

export const inc = m.action(Big, "one up", { when: (s) => s.n < 2147483650, changes: (s) => ({ n: s.n + 1 }) });

export const top = m.end(Big, "At the top", (s) => s.n === 2147483650);

export const low = m.rule(Big, "Below the top", (s) => s.n < 2147483649);
export const prod = m.rule(Big, "Product", (s) => s.n * 2 > 0);
