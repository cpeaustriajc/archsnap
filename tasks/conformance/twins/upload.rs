use earnest::model::{Model, Request, Retry, Timer, model, require};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
struct Upload {
    call: Request,
    backoff: Timer,
    tries: Retry<2>,
}

#[model]
impl Upload {
    #[action("the app sends the file")]
    fn send(&self) -> Option<Self> {
        require!(self.call.is_idle() && self.backoff.is_idle());
        Some(Self {
            call: self.call.send(),
            ..self.clone()
        })
    }

    #[action("the server accepts the file")]
    fn succeed(&self) -> Option<Self> {
        require!(self.call.is_pending());
        Some(Self {
            call: self.call.succeed(),
            ..self.clone()
        })
    }

    #[action("the server rejects the file")]
    fn fail(&self) -> Option<Self> {
        require!(self.call.is_pending());
        Some(Self {
            call: self.call.fail(),
            ..self.clone()
        })
    }

    #[action("the app backs off before another try")]
    fn wait(&self) -> Option<Self> {
        require!(self.call.has_failed() && self.tries.can_retry() && self.backoff.is_idle());
        Some(Self {
            call: self.call.reset(),
            backoff: self.backoff.start(),
            tries: self.tries.again(),
        })
    }

    #[action("the backoff timer fires")]
    fn wake(&self) -> Option<Self> {
        require!(self.backoff.is_running());
        Some(Self {
            backoff: self.backoff.fire(),
            ..self.clone()
        })
    }

    #[action("the app clears the fired timer")]
    fn rearm(&self) -> Option<Self> {
        require!(self.backoff.has_fired());
        Some(Self {
            backoff: self.backoff.reset(),
            ..self.clone()
        })
    }

    #[action("the user starts over while the app waits")]
    fn abort(&self) -> Option<Self> {
        require!(self.backoff.is_running());
        Some(Self {
            backoff: self.backoff.cancel(),
            tries: self.tries.reset(),
            ..self.clone()
        })
    }

    #[rule("A settled call has no timer running")]
    fn quiet(&self) -> bool {
        !self.call.is_settled() || self.backoff.is_idle()
    }

    #[rule("While the timer is not idle the call is idle")]
    fn waiting(&self) -> bool {
        self.backoff.is_idle() || self.call.is_idle()
    }

    #[rule("The tries never pass the maximum")]
    fn counted(&self) -> bool {
        self.tries.attempts() <= 2 && (!self.tries.is_exhausted() || !self.tries.can_retry())
    }

    #[end("The file is uploaded")]
    fn sent(&self) -> bool {
        self.call.has_succeeded()
    }

    #[end("Every try failed")]
    fn spent(&self) -> bool {
        self.call.has_failed() && self.tries.is_exhausted()
    }
}
