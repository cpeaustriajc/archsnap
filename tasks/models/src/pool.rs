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
    states: u8,
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
    #[initial(Task { phase: Phase::Waiting, levels: 0, loan: 0, busy: 0, states: 0 })]
    a: Task,
    #[initial(Task { phase: Phase::Waiting, levels: 0, loan: 0, busy: 0, states: 0 })]
    b: Task,
    #[initial(Task { phase: Phase::Waiting, levels: 0, loan: 0, busy: 0, states: 0 })]
    c: Task,
}

fn absent() -> Task {
    Task {
        phase: Phase::Absent,
        levels: 0,
        loan: 0,
        busy: 0,
        states: 0,
    }
}

fn share(states: u8, jobs: u8) -> u8 {
    if jobs >= states {
        1
    } else if 2 * jobs >= states {
        2
    } else if 3 * jobs >= states {
        3
    } else {
        4
    }
}

fn chunks(states: u8, jobs: u8) -> u8 {
    let size = share(states, jobs);
    if size >= states {
        1
    } else if 2 * size >= states {
        2
    } else if 3 * size >= states {
        3
    } else {
        4
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
    task.loan == 0 || task.loan + OWN_THREADS <= task.states
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
            ..self.clone()
        })
    }

    #[action("a model starts a level and borrows idle threads for it")]
    fn start_level(&self, job: Job, #[range(1, 4)] states: u8, level: Level) -> Option<Self> {
        let task = self.task(job);
        require!(task.phase == Phase::Between && task.levels < MOST_LEVELS);
        let wanted = if level == Level::WorthSpreading {
            states - OWN_THREADS
        } else {
            0
        };
        let loan = smaller(self.idle, wanted);
        let busy = chunks(states, OWN_THREADS + loan) - OWN_THREADS;
        let changed = Task {
            phase: Phase::Spreading,
            levels: task.levels + 1,
            loan,
            busy,
            states,
        };
        Some(Self {
            idle: self.idle - loan,
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
            ..self.clone()
        })
    }

    #[action("a model finishes its level and gives the borrowed threads back")]
    fn end_level(&self, job: Job) -> Option<Self> {
        let task = self.task(job);
        require!(task.phase == Phase::Spreading);
        let changed = Task {
            phase: Phase::Between,
            loan: 0,
            busy: 0,
            states: 0,
            ..task
        };
        Some(Self {
            idle: self.idle + task.loan,
            a: pick(job, Job::A, &changed, &self.a),
            b: pick(job, Job::B, &changed, &self.b),
            c: pick(job, Job::C, &changed, &self.c),
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

    #[rule("A level runs no more helper threads than it borrowed")]
    fn runs_what_it_borrowed(&self) -> bool {
        self.a.busy <= self.a.loan && self.b.busy <= self.b.loan && self.c.busy <= self.c.loan
    }

    #[rule("A level borrows no more threads than it has states to share out")]
    fn borrows_what_it_can_share(&self) -> bool {
        borrows_within(&self.a) && borrows_within(&self.b) && borrows_within(&self.c)
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
                states: 1702,
                broken: Vec::new(),
                stuck: 0,
            },
            "the model run as Rust agrees with the count in README.md"
        );
    }
}
