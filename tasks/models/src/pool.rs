use earnest::model::{Domain, Model, model, require, view};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Setup {
    TwoThreadsThreeModels,
    ThreeThreadsTwoModels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Phase {
    Absent,
    Waiting,
    Between,
    Spreading,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Job {
    A,
    B,
    C,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Level {
    NotWorthSpreading,
    WorthSpreading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Stage {
    NoPoolYet,
    ModelsWait,
    AllStarted,
    AllFinished,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Task {
    phase: Phase,
    #[range(0, 2)]
    levels: u8,
    #[range(0, 2)]
    loan: u8,
    #[range(0, 2)]
    busy: u8,
    #[range(0, 4)]
    parts: u8,
    #[range(0, 4)]
    taken: u8,
    spread: bool,
    polled: bool,
}

const MOST_LEVELS: u8 = 2;
const OWN_THREADS: u8 = 1;
const ONE: u8 = 1;
const NONE: u8 = 0;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Pool {
    setup: Setup,
    #[initial(false)]
    opened: bool,
    #[range(0, 3)]
    #[initial(0)]
    idle: u8,
    #[range(0, 5)]
    #[initial(0)]
    next: u8,
    #[range(0, 3)]
    #[initial(0)]
    claiming: u8,
    #[range(0, 3)]
    #[initial(0)]
    leaving: u8,
    #[initial(Task { phase: Phase::Waiting, levels: 0, loan: 0, busy: 0, parts: 0, taken: 0, spread: false, polled: false })]
    a: Task,
    #[initial(Task { phase: Phase::Waiting, levels: 0, loan: 0, busy: 0, parts: 0, taken: 0, spread: false, polled: false })]
    b: Task,
    #[initial(Task { phase: Phase::Waiting, levels: 0, loan: 0, busy: 0, parts: 0, taken: 0, spread: false, polled: false })]
    c: Task,
}

fn absent() -> Task {
    Task {
        phase: Phase::Absent,
        levels: 0,
        loan: 0,
        busy: 0,
        parts: 0,
        taken: 0,
        spread: false,
        polled: false,
    }
}

fn smaller(left: u8, right: u8) -> u8 {
    if left < right { left } else { right }
}

fn running(task: &Task) -> u8 {
    if task.phase == Phase::Between || task.phase == Phase::Spreading {
        ONE
    } else {
        NONE
    }
}

fn pick(job: Job, slot: Job, changed: &Task, kept: &Task) -> Task {
    if job == slot {
        changed.clone()
    } else {
        kept.clone()
    }
}

fn lent(pool: &Pool) -> u8 {
    pool.a.loan + pool.b.loan + pool.c.loan
}

fn on_their_own_model(pool: &Pool) -> u8 {
    running(&pool.a) + running(&pool.b) + running(&pool.c)
}

fn waits(task: &Task) -> u8 {
    if task.phase == Phase::Waiting {
        ONE
    } else {
        NONE
    }
}

fn waiting(pool: &Pool) -> u8 {
    waits(&pool.a) + waits(&pool.b) + waits(&pool.c)
}

fn stage(pool: &Pool) -> Stage {
    if !pool.opened {
        Stage::NoPoolYet
    } else if waiting(pool) > 0 {
        Stage::ModelsWait
    } else if on_their_own_model(pool) > 0 {
        Stage::AllStarted
    } else {
        Stage::AllFinished
    }
}

fn borrows_within(task: &Task) -> bool {
    task.loan == 0 || task.loan + OWN_THREADS <= task.parts
}

fn could_use_another(task: &Task) -> bool {
    task.phase == Phase::Spreading
        && task.spread
        && task.taken < task.parts
        && task.loan + OWN_THREADS < task.parts
}

fn looked_past_an_idle_thread(task: &Task) -> bool {
    task.polled && could_use_another(task)
}

fn unseen(task: &Task) -> Task {
    Task {
        polled: false,
        ..task.clone()
    }
}

fn holds_nothing(task: &Task) -> bool {
    !finished(task) || task.loan == 0
}

fn finished(task: &Task) -> bool {
    task.phase == Phase::Absent || task.phase == Phase::Done
}

#[model]
impl Pool {
    fn threads(&self) -> u8 {
        if self.setup == Setup::TwoThreadsThreeModels {
            2
        } else {
            3
        }
    }

    fn tasks(&self) -> u8 {
        if self.setup == Setup::TwoThreadsThreeModels {
            3
        } else {
            2
        }
    }

    fn task(&self, job: Job) -> Task {
        if job == Job::A {
            self.a.clone()
        } else if job == Job::B {
            self.b.clone()
        } else {
            self.c.clone()
        }
    }

    fn accounted(&self) -> u8 {
        self.idle + lent(self) + self.claiming + self.leaving + on_their_own_model(self)
    }

    #[action("the run makes its pool: a crew of one thread per model, the rest idle")]
    fn open(&self) -> Option<Self> {
        require!(!self.opened);
        let crew = smaller(self.threads(), self.tasks());
        Some(Self {
            opened: true,
            idle: self.threads() - crew,
            claiming: crew,
            c: if self.tasks() == 3 {
                self.c.clone()
            } else {
                absent()
            },
            ..self.clone()
        })
    }

    #[action("a thread of the crew claims the next model")]
    fn claim(&self) -> Option<Self> {
        require!(self.claiming > 0 && self.next < self.tasks());
        let job = if self.next == 0 {
            Job::A
        } else if self.next == 1 {
            Job::B
        } else {
            Job::C
        };
        let changed = Task {
            phase: Phase::Between,
            ..self.task(job)
        };
        Some(Self {
            next: self.next + 1,
            claiming: self.claiming - 1,
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
            ..self.clone()
        })
    }

    #[action("a thread of the crew finds no model left")]
    fn find_none(&self) -> Option<Self> {
        require!(self.claiming > 0 && self.next >= self.tasks());
        Some(Self {
            next: self.next + 1,
            claiming: self.claiming - 1,
            leaving: self.leaving + 1,
            ..self.clone()
        })
    }

    #[action("a thread that found no model gives itself to the idle threads")]
    fn retire(&self) -> Option<Self> {
        require!(self.leaving > 0);
        Some(Self {
            leaving: self.leaving - 1,
            idle: self.idle + 1,
            a: unseen(&self.a),
            b: unseen(&self.b),
            c: unseen(&self.c),
            ..self.clone()
        })
    }

    #[action("a model starts a level and borrows idle threads for it")]
    fn start_level(&self, job: Job, #[range(1, 4)] parts: u8, level: Level) -> Option<Self> {
        let task = self.task(job);
        require!(task.phase == Phase::Between && task.levels < MOST_LEVELS);
        let spread = level == Level::WorthSpreading;
        let wanted: u8 = if spread { parts - OWN_THREADS } else { 0 };
        let loan = smaller(self.idle, wanted);
        let changed = Task {
            phase: Phase::Spreading,
            levels: task.levels + 1,
            loan,
            busy: loan,
            parts,
            taken: NONE,
            spread,
            polled: true,
        };
        Some(Self {
            idle: self.idle - loan,
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
            ..self.clone()
        })
    }

    #[action("a running level looks at the idle threads and takes up as many as it has parts for")]
    fn poll(&self, job: Job) -> Option<Self> {
        let task = self.task(job);
        require!(task.phase == Phase::Spreading && task.spread && !task.polled);
        let joining = smaller(self.idle, task.parts - OWN_THREADS - task.loan);
        let changed = Task {
            loan: task.loan + joining,
            busy: task.busy + joining,
            polled: true,
            ..task
        };
        Some(Self {
            idle: self.idle - joining,
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
            ..self.clone()
        })
    }

    #[action(
        "a thread of a running level takes the next part, after the level has looked at the idle threads"
    )]
    fn take_part(&self, job: Job) -> Option<Self> {
        let task = self.task(job);
        require!(
            task.phase == Phase::Spreading && task.spread && task.polled && task.taken < task.parts
        );
        let changed = Task {
            taken: task.taken + 1,
            polled: false,
            ..task
        };
        Some(Self {
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
            ..self.clone()
        })
    }

    #[action("a model finishes its level and gives the borrowed threads back")]
    fn end_level(&self, job: Job) -> Option<Self> {
        let task = self.task(job);
        require!(task.phase == Phase::Spreading && (!task.spread || task.taken == task.parts));
        let changed = Task {
            phase: Phase::Between,
            loan: 0,
            busy: 0,
            parts: 0,
            taken: 0,
            spread: false,
            polled: false,
            ..task
        };
        Some(Self {
            idle: self.idle + task.loan,
            a: if job == Job::A {
                changed.clone()
            } else {
                unseen(&self.a)
            },
            b: if job == Job::B {
                changed.clone()
            } else {
                unseen(&self.b)
            },
            c: if job == Job::C {
                changed
            } else {
                unseen(&self.c)
            },
            ..self.clone()
        })
    }

    #[action("a model's search ends and its thread looks for the next model")]
    fn finish(&self, job: Job) -> Option<Self> {
        let task = self.task(job);
        require!(task.phase == Phase::Between && task.levels > 0);
        let changed = Task {
            phase: Phase::Done,
            ..task
        };
        Some(Self {
            claiming: self.claiming + 1,
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
            ..self.clone()
        })
    }

    #[rule("No thread is counted in two places, so none is lent to two models at once")]
    fn lent_once(&self) -> bool {
        self.accounted() <= self.threads()
    }

    #[rule("No thread is lost: idle, lent and working threads add up to all of them")]
    fn none_lost(&self) -> bool {
        !self.opened || self.accounted() >= self.threads()
    }

    #[rule("Every running model holds its own thread, so none runs on zero threads")]
    fn own_thread(&self) -> bool {
        on_their_own_model(self) + self.claiming + self.leaving
            <= smaller(self.threads(), self.tasks())
    }

    #[rule(
        "While a model waits with no thread of the crew coming for it, no thread is idle or lent"
    )]
    fn none_idle_while_waiting(&self) -> bool {
        waiting(self) <= self.claiming || self.idle + lent(self) == 0
    }

    #[rule("Every thread a level borrowed runs a part of it")]
    fn runs_what_it_borrowed(&self) -> bool {
        self.a.busy == self.a.loan && self.b.busy == self.b.loan && self.c.busy == self.c.loan
    }

    #[rule("A level borrows no more threads than it has states to share out")]
    fn borrows_what_it_can_share(&self) -> bool {
        borrows_within(&self.a) && borrows_within(&self.b) && borrows_within(&self.c)
    }

    #[rule(
        "No thread stays idle beside a running level that has looked at the idle threads and could use it"
    )]
    fn none_idle_beside_a_level(&self) -> bool {
        self.idle == 0
            || !(looked_past_an_idle_thread(&self.a)
                || looked_past_an_idle_thread(&self.b)
                || looked_past_an_idle_thread(&self.c))
    }

    #[rule("A finished model holds no borrowed thread")]
    fn finished_holds_nothing(&self) -> bool {
        holds_nothing(&self.a) && holds_nothing(&self.b) && holds_nothing(&self.c)
    }

    #[end("Every model is finished and every thread is back with the idle threads")]
    fn all_back(&self) -> bool {
        finished(&self.a) && finished(&self.b) && finished(&self.c) && self.idle == self.threads()
    }
}

#[view(
    title = "Shared thread pool",
    step(
        open = "make the pool",
        claim = "claim",
        find_none = "nothing left",
        retire = "retire",
        start_level = "borrow",
        poll = "take idle threads",
        take_part = "take a part",
        end_level = "give back",
        finish = "model done"
    )
)]
impl Pool {
    #[state]
    fn state(&self) -> String {
        format!("{:?}, {} idle, {} lent", stage(self), self.idle, lent(self))
    }

    #[group]
    fn group(&self) -> String {
        format!("{:?}", self.setup)
    }
}

#[cfg(test)]
mod tests {
    use super::Pool;
    use crate::walk::{Walk, Walked, walk};
    use earnest::model::{Behavior, Model};

    #[test]
    fn rustc_reaches_the_states_the_checker_counts() {
        let walked = walk(&Walk {
            starts: Pool::starts(),
            next: |state| state.steps().into_iter().map(|step| step.state).collect(),
            broken: |state| {
                state
                    .rules()
                    .into_iter()
                    .filter(|rule| !rule.holds)
                    .map(|rule| rule.id)
                    .collect()
            },
            ends: |state| state.ends().iter().any(|end| end.holds),
        });
        assert_eq!(
            walked,
            Walked {
                states: 35612,
                broken: Vec::new(),
                stuck: 0,
            },
            "the model run as Rust agrees with the count in README.md"
        );
    }
}
