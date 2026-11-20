use earnest::model::{Domain, Model, model, view};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Content {
    First,
    Second,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum MaxStates {
    Default,
    Small,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum TimeLimit {
    Unlimited,
    Seconds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Timing {
    NoLimit,
    BothFinish,
    AStops,
    BStops,
    BothStop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Picked {
    Every,
    OnlyB,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Only {
    EveryRule,
    RuleOfA,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Build {
    Old,
    Rebuilt,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Sources {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Caching {
    Used,
    NoCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Noise {
    Nothing,
    OtherJobs,
    Touched,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Job {
    A,
    B,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Seen {
    Nothing,
    Refused,
    Ran,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
enum Served {
    Unchecked,
    Fresh,
    Cached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Flags {
    source: Content,
    max_states: MaxStates,
    timing: Timing,
    picked: Picked,
    only: Only,
    build: Build,
    caching: Caching,
    noise: Noise,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Key {
    engine: Build,
    source: Content,
    max_states: MaxStates,
    time_limit: TimeLimit,
    rule: Only,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Asked {
    sources: Sources,
    source: Content,
    max_states: MaxStates,
    time_limit: TimeLimit,
    rule: Only,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Made {
    model: Job,
    sources: Sources,
    source: Content,
    max_states: MaxStates,
    rule: Only,
    timed_out: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Domain)]
struct Pass {
    old: Option<Made>,
    from: Option<Made>,
    fresh: Option<Made>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Cache {
    #[initial(Seen::Nothing)]
    seen: Seen,
    #[initial(None)]
    entry: Option<Key>,
    #[initial(None)]
    stored_a: Option<Made>,
    #[initial(None)]
    stored_b: Option<Made>,
    #[initial(None)]
    asked: Option<Asked>,
    #[initial(Served::Unchecked)]
    served_a: Served,
    #[initial(None)]
    shown_a: Option<Made>,
    #[initial(Served::Unchecked)]
    served_b: Served,
    #[initial(None)]
    shown_b: Option<Made>,
    #[initial(None)]
    lasting_a: Option<Asked>,
    #[initial(None)]
    lasting_b: Option<Asked>,
    #[initial(false)]
    due_a: bool,
    #[initial(false)]
    due_b: bool,
    #[initial(false)]
    forced: bool,
}

fn sources_of(build: Build) -> Sources {
    if build == Build::New {
        Sources::After
    } else {
        Sources::Before
    }
}

fn fingerprint(build: Build) -> Build {
    if build == Build::Rebuilt {
        Build::Old
    } else {
        build
    }
}

fn limit_of(timing: Timing) -> TimeLimit {
    if timing == Timing::NoLimit {
        TimeLimit::Unlimited
    } else {
        TimeLimit::Seconds
    }
}

fn asked_of(flags: Flags) -> Asked {
    Asked {
        sources: sources_of(flags.build),
        source: flags.source,
        max_states: flags.max_states,
        time_limit: limit_of(flags.timing),
        rule: flags.only,
    }
}

fn refused(flags: Flags) -> bool {
    flags.only == Only::RuleOfA && flags.picked == Picked::OnlyB
}

fn checks(job: Job, flags: Flags) -> bool {
    if job == Job::A {
        flags.picked == Picked::Every
    } else {
        flags.only == Only::EveryRule
    }
}

fn key_of(flags: Flags) -> Key {
    Key {
        engine: fingerprint(flags.build),
        source: flags.source,
        max_states: flags.max_states,
        time_limit: limit_of(flags.timing),
        rule: flags.only,
    }
}

fn stops(job: Job, timing: Timing) -> bool {
    timing == Timing::BothStop
        || (job == Job::A && timing == Timing::AStops)
        || (job == Job::B && timing == Timing::BStops)
}

fn fresh(job: Job, flags: Flags) -> Made {
    Made {
        model: job,
        sources: sources_of(flags.build),
        source: flags.source,
        max_states: flags.max_states,
        rule: flags.only,
        timed_out: stops(job, flags.timing),
    }
}

fn lasting(key: &Key, made: &Made) -> bool {
    !(key.time_limit == TimeLimit::Seconds && made.timed_out)
}

fn answers(made: &Made, job: Job, asked: &Asked) -> bool {
    made.model == job
        && made.sources == asked.sources
        && made.source == asked.source
        && made.max_states == asked.max_states
        && made.rule == asked.rule
}

fn stored_of(cache: &Cache, job: Job) -> Option<Made> {
    if job == Job::A {
        cache.stored_a.clone()
    } else {
        cache.stored_b.clone()
    }
}

fn shown_of(cache: &Cache, job: Job) -> Option<Made> {
    if job == Job::A {
        cache.shown_a.clone()
    } else {
        cache.shown_b.clone()
    }
}

fn served_of(cache: &Cache, job: Job) -> Served {
    if job == Job::A {
        cache.served_a
    } else {
        cache.served_b
    }
}

fn lasting_of(cache: &Cache, job: Job) -> Option<Asked> {
    if job == Job::A {
        cache.lasting_a.clone()
    } else {
        cache.lasting_b.clone()
    }
}

fn due_of(cache: &Cache, job: Job) -> bool {
    if job == Job::A {
        cache.due_a
    } else {
        cache.due_b
    }
}

fn loaded(cache: &Cache, job: Job, flags: Flags) -> Option<Made> {
    if cache.entry == Some(key_of(flags)) {
        stored_of(cache, job)
    } else {
        None
    }
}

fn pass_for(cache: &Cache, job: Job, flags: Flags) -> Pass {
    let old = loaded(cache, job, flags);
    let from = if checks(job, flags) && flags.caching == Caching::Used {
        old.clone()
    } else {
        None
    };
    let fresh = if checks(job, flags) && from.is_none() {
        Some(fresh(job, flags))
    } else {
        None
    };
    Pass { old, from, fresh }
}

fn kept(pass: &Pass, key: &Key) -> Option<Made> {
    if pass.fresh.is_none() {
        pass.old.clone()
    } else if pass.fresh.clone().is_some_and(|made| lasting(key, &made)) {
        pass.fresh.clone()
    } else {
        None
    }
}

fn served(pass: &Pass) -> Served {
    if pass.from.is_some() {
        Served::Cached
    } else if pass.fresh.is_some() {
        Served::Fresh
    } else {
        Served::Unchecked
    }
}

fn shown(pass: &Pass) -> Option<Made> {
    if pass.from.is_some() {
        pass.from.clone()
    } else {
        pass.fresh.clone()
    }
}

fn stopped_early(made: &Made) -> bool {
    made.timed_out
}

fn lasting_after(cache: &Cache, job: Job, flags: Flags, pass: &Pass) -> Option<Asked> {
    let asked = asked_of(flags);
    let last = lasting_of(cache, job);
    if !checks(job, flags) {
        if last == Some(asked) { last } else { None }
    } else if pass.fresh.clone().is_some_and(|made| stopped_early(&made)) {
        None
    } else {
        Some(asked)
    }
}

fn due(cache: &Cache, job: Job, flags: Flags) -> bool {
    checks(job, flags)
        && flags.caching == Caching::Used
        && lasting_of(cache, job) == Some(asked_of(flags))
}

fn served_right(cache: &Cache, job: Job) -> bool {
    served_of(cache, job) != Served::Cached
        || shown_of(cache, job).is_some_and(|made| {
            !made.timed_out
                && cache
                    .asked
                    .clone()
                    .is_some_and(|asked| answers(&made, job, &asked))
        })
}

fn reused(cache: &Cache, job: Job) -> bool {
    !due_of(cache, job) || served_of(cache, job) == Served::Cached
}

fn never_timed_out(cache: &Cache, job: Job) -> bool {
    !stored_of(cache, job).is_some_and(|made| made.timed_out)
}

#[model]
impl Cache {
    #[action("earnest verify runs on the file with these flags and this build")]
    fn run(&self, flags: Flags) -> Self {
        if refused(flags) {
            return Self {
                seen: Seen::Refused,
                asked: None,
                served_a: Served::Unchecked,
                shown_a: None,
                served_b: Served::Unchecked,
                shown_b: None,
                due_a: false,
                due_b: false,
                forced: false,
                ..self.clone()
            };
        }
        let key = key_of(flags);
        let a = pass_for(self, Job::A, flags);
        let b = pass_for(self, Job::B, flags);
        let stores = a.fresh.is_some() || b.fresh.is_some();
        Self {
            seen: Seen::Ran,
            entry: if stores {
                Some(key.clone())
            } else {
                self.entry.clone()
            },
            stored_a: if stores {
                kept(&a, &key)
            } else {
                self.stored_a.clone()
            },
            stored_b: if stores {
                kept(&b, &key)
            } else {
                self.stored_b.clone()
            },
            asked: Some(asked_of(flags)),
            served_a: served(&a),
            shown_a: shown(&a),
            served_b: served(&b),
            shown_b: shown(&b),
            lasting_a: lasting_after(self, Job::A, flags, &a),
            lasting_b: lasting_after(self, Job::B, flags, &b),
            due_a: due(self, Job::A, flags),
            due_b: due(self, Job::B, flags),
            forced: flags.caching == Caching::NoCache,
        }
    }

    #[rule(
        "A cached result is served only for the same model, source, build, state limit and rule, and never one a time limit stopped"
    )]
    fn served_for_the_same_input(&self) -> bool {
        served_right(self, Job::A) && served_right(self, Job::B)
    }

    #[rule("A run with --no-cache serves nothing from the cache")]
    fn no_cache_checks_again(&self) -> bool {
        !self.forced || (self.served_a != Served::Cached && self.served_b != Served::Cached)
    }

    #[rule("A result that a time limit stopped is never stored")]
    fn timed_out_never_stored(&self) -> bool {
        never_timed_out(self, Job::A) && never_timed_out(self, Job::B)
    }

    #[rule(
        "A run that differs from the last check of a model only in what can't change its result is served from the cache"
    )]
    fn reused_when_nothing_changed(&self) -> bool {
        reused(self, Job::A) && reused(self, Job::B)
    }
}

#[view(title = "Result cache", step(run = "verify"))]
impl Cache {
    #[group]
    fn group(&self) -> String {
        format!("{:?}", self.seen)
    }

    #[state]
    fn state(&self) -> String {
        format!("A {:?}, B {:?}", self.served_a, self.served_b)
    }
}

#[cfg(test)]
mod tests {
    use super::Cache;
    use crate::walk::{Walk, Walked, walk};
    use earnest::model::{Behavior, Model};

    #[test]
    fn rustc_reaches_the_states_the_checker_counts() {
        let walked = walk(&Walk {
            starts: Cache::starts(),
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
                states: 394,
                broken: Vec::new(),
                stuck: 0,
            },
            "the model run as Rust agrees with the count in README.md"
        );
    }
}
