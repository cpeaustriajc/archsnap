use earnest_check::{Helpers, WORKER_STACK};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

#[derive(Debug)]
struct Pool {
    tasks: usize,
    crew: usize,
    next: AtomicUsize,
    helpers: Helpers,
}

impl Pool {
    fn new(threads: usize, tasks: usize) -> Self {
        let threads = threads.max(1);
        let crew = threads.min(tasks);
        Self {
            tasks,
            crew,
            next: AtomicUsize::new(0),
            helpers: Helpers::new(threads.saturating_sub(crew)),
        }
    }

    fn claim(&self) -> Option<usize> {
        let task = self.next.fetch_add(1, Ordering::SeqCst);
        (task < self.tasks).then_some(task)
    }

    fn retire(&self) {
        self.helpers.add(1);
    }
}

/// Runs `work` once on every task, on `threads` threads in all, and returns the results in task
/// order.
///
/// Each thread of the crew runs one task at a time, so no more tasks run side by side than
/// there are threads. `work` also gets the [`Helpers`] of the run: the threads that no task is
/// using, which a task can borrow for the next stretch of work it can spread, such as one level
/// of states. A thread that finds no task left joins them, so the threads a finished task
/// leaves behind go to the tasks that are still running, also to one that started late. The
/// results do not depend on `threads`.
pub(crate) fn run_each<T, R, F>(tasks: &[T], threads: usize, work: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T, &Helpers) -> R + Sync,
{
    let pool = Pool::new(threads, tasks.len());
    let mut done: Vec<(usize, R)> = thread::scope(|scope| {
        let workers: Vec<_> = (0..pool.crew)
            .map(|_| {
                thread::Builder::new()
                    .stack_size(WORKER_STACK)
                    .spawn_scoped(scope, || {
                        let mut mine = Vec::new();
                        while let Some(position) = pool.claim() {
                            if let Some(task) = tasks.get(position) {
                                mine.push((position, work(task, &pool.helpers)));
                            }
                        }
                        pool.retire();
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
    use super::{Pool, run_each};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

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
    fn the_threads_in_use_never_exceed_the_limit() {
        for (threads, count) in [(4, 1), (4, 2), (4, 3), (4, 9), (3, 7), (1, 5), (8, 3)] {
            let tasks: Vec<usize> = (0..count).collect();
            let in_use = AtomicUsize::new(0);
            let most = AtomicUsize::new(0);
            let shares = run_each(&tasks, threads, |_, helpers| {
                let loan = helpers.borrow(usize::MAX);
                let share = 1 + loan.threads();
                let now = in_use.fetch_add(share, Ordering::SeqCst) + share;
                most.fetch_max(now, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(2));
                in_use.fetch_sub(share, Ordering::SeqCst);
                share
            });
            assert!(
                shares.iter().all(|share| *share >= 1),
                "every task has a thread with {threads} threads and {count} tasks"
            );
            assert!(
                most.load(Ordering::SeqCst) <= threads,
                "{} threads in use with a limit of {threads} and {count} tasks",
                most.load(Ordering::SeqCst)
            );
        }
    }

    #[test]
    fn a_lone_task_can_borrow_every_other_thread() {
        let shares = run_each(&[()], 6, |(), helpers| {
            1 + helpers.borrow(usize::MAX).threads()
        });
        assert_eq!(
            shares,
            [6],
            "one model can spread its levels over all threads"
        );
    }

    #[test]
    fn the_crew_is_as_big_as_the_threads_or_the_tasks_and_the_rest_are_idle() {
        for (threads, tasks, crew, idle) in [
            (4, 1, 1, 3),
            (4, 3, 3, 1),
            (4, 4, 4, 0),
            (4, 9, 4, 0),
            (0, 2, 1, 0),
            (1, 0, 0, 1),
        ] {
            let pool = Pool::new(threads, tasks);
            assert_eq!(
                (pool.crew, pool.helpers.idle()),
                (crew, idle),
                "{threads} threads and {tasks} tasks"
            );
        }
    }

    #[test]
    fn tasks_are_claimed_once_each_in_order_and_then_none() {
        let pool = Pool::new(2, 3);
        let claimed: Vec<Option<usize>> = (0..5).map(|_| pool.claim()).collect();
        assert_eq!(
            claimed,
            [Some(0), Some(1), Some(2), None, None],
            "in order, then nothing"
        );
    }

    #[test]
    fn a_task_that_starts_late_can_borrow_the_threads_the_finished_tasks_left() {
        let pool = Pool::new(4, 3);
        let claimed: Vec<Option<usize>> = (0..3).map(|_| pool.claim()).collect();
        assert_eq!(claimed, [Some(0), Some(1), Some(2)], "all three run");
        assert_eq!(pool.helpers.idle(), 1, "one thread has no task");
        assert_eq!(
            pool.claim(),
            None,
            "the first thread to finish finds nothing left"
        );
        pool.retire();
        assert_eq!(pool.claim(), None, "and so does the second");
        pool.retire();
        let late = pool.helpers.borrow(usize::MAX);
        assert_eq!(
            1 + late.threads(),
            4,
            "the last task spreads over its own thread and the three that are idle"
        );
    }

    #[test]
    fn while_tasks_wait_no_thread_is_idle() {
        let pool = Pool::new(4, 13);
        assert_eq!(
            pool.helpers.borrow(usize::MAX).threads(),
            0,
            "every thread runs a task, so a model that starts early has one thread"
        );
    }
}
