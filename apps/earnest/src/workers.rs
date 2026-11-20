use earnest_check::WORKER_STACK;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;

#[derive(Debug, Default)]
struct Desk {
    next: usize,
    busy: usize,
}

#[derive(Debug)]
struct Pool {
    threads: usize,
    tasks: usize,
    desk: Mutex<Desk>,
    freed: Condvar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Share {
    task: usize,
    threads: usize,
}

fn threads_for_next(threads: usize, busy: usize, waiting: usize) -> usize {
    let spare = threads.saturating_sub(busy).saturating_sub(1);
    let kept_for_others = waiting.min(spare);
    spare.saturating_sub(kept_for_others).saturating_add(1)
}

impl Pool {
    fn desk(&self) -> MutexGuard<'_, Desk> {
        self.desk.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn take(&self) -> Option<Share> {
        let mut desk = self.desk();
        loop {
            if desk.next >= self.tasks {
                return None;
            }
            if desk.busy < self.threads {
                break;
            }
            desk = self
                .freed
                .wait(desk)
                .unwrap_or_else(PoisonError::into_inner);
        }
        let task = desk.next;
        desk.next = task.saturating_add(1);
        let waiting = self.tasks.saturating_sub(desk.next);
        let threads = threads_for_next(self.threads, desk.busy, waiting);
        desk.busy = desk.busy.saturating_add(threads);
        Some(Share { task, threads })
    }

    fn give_back(&self, share: Share) {
        let mut desk = self.desk();
        desk.busy = desk.busy.saturating_sub(share.threads);
        drop(desk);
        self.freed.notify_all();
    }
}

pub(crate) fn run_each<T, R, F>(tasks: &[T], threads: usize, work: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T, usize) -> R + Sync,
{
    let pool = Pool {
        threads: threads.max(1),
        tasks: tasks.len(),
        desk: Mutex::new(Desk::default()),
        freed: Condvar::new(),
    };
    let crew = pool.threads.min(tasks.len());
    let mut done: Vec<(usize, R)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..crew)
            .map(|_| {
                thread::Builder::new()
                    .stack_size(WORKER_STACK)
                    .spawn_scoped(scope, || {
                        let mut mine = Vec::new();
                        while let Some(share) = pool.take() {
                            if let Some(task) = tasks.get(share.task) {
                                mine.push((share.task, work(task, share.threads)));
                            }
                            pool.give_back(share);
                        }
                        mine
                    })
                    .expect("a worker thread starts")
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| match worker.join() {
                Ok(mine) => mine,
                Err(panic) => std::panic::resume_unwind(panic),
            })
            .collect()
    });
    done.sort_by_key(|(task, _)| *task);
    done.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::{run_each, threads_for_next};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn one_task_takes_every_thread() {
        assert_eq!(threads_for_next(4, 0, 0), 4, "nothing else is waiting");
        assert_eq!(threads_for_next(1, 0, 0), 1, "one thread is one thread");
    }

    #[test]
    fn a_task_leaves_one_thread_for_each_task_still_waiting() {
        assert_eq!(threads_for_next(4, 0, 1), 3, "one waits");
        assert_eq!(threads_for_next(4, 0, 3), 1, "three wait");
        assert_eq!(
            threads_for_next(4, 0, 20),
            1,
            "more wait than there are threads"
        );
        assert_eq!(threads_for_next(4, 3, 0), 1, "the others are busy");
        assert_eq!(threads_for_next(8, 2, 1), 5, "two busy, one waits");
    }

    #[test]
    fn results_come_back_in_task_order_for_every_thread_count() {
        let tasks: Vec<usize> = (0..25).collect();
        let squares: Vec<usize> = tasks.iter().map(|task| task * task).collect();
        for threads in [0, 1, 2, 3, 4, 8, 64] {
            let found = run_each(&tasks, threads, |task, _| {
                thread::sleep(Duration::from_millis(u64::try_from(25 - task).unwrap() % 3));
                task * task
            });
            assert_eq!(found, squares, "with {threads} threads");
        }
    }

    #[test]
    fn no_tasks_give_no_results() {
        let nothing: [usize; 0] = [];
        let found = run_each(&nothing, 4, |task, _| *task);
        assert!(found.is_empty(), "nothing to run");
    }

    #[test]
    fn the_threads_handed_out_never_exceed_the_limit() {
        for (threads, count) in [(4, 1), (4, 2), (4, 3), (4, 9), (3, 7), (1, 5), (8, 3)] {
            let tasks: Vec<usize> = (0..count).collect();
            let in_use = AtomicUsize::new(0);
            let most = AtomicUsize::new(0);
            let shares = run_each(&tasks, threads, |_, share| {
                let now = in_use.fetch_add(share, Ordering::SeqCst) + share;
                most.fetch_max(now, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(2));
                in_use.fetch_sub(share, Ordering::SeqCst);
                share
            });
            assert!(
                shares.iter().all(|share| *share >= 1),
                "every task gets a thread with {threads} threads and {count} tasks"
            );
            assert!(
                most.load(Ordering::SeqCst) <= threads,
                "{} threads in use with a limit of {threads} and {count} tasks",
                most.load(Ordering::SeqCst)
            );
        }
    }

    #[test]
    fn a_lone_task_is_given_every_thread() {
        let shares = run_each(&[()], 6, |(), share| share);
        assert_eq!(
            shares,
            [6],
            "one model can split its levels over all workers"
        );
    }
}
