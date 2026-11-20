use crate::build::{Sketch, below, is, not, plus};
use earnest_check::{Helpers, Limits, explore};
use earnest_ir::{Expr, Model};
use std::thread;
use std::time::Duration;

fn cube() -> Model {
    let mut sketch = Sketch::new();
    let axes: Vec<Expr> = ["x", "y", "z"]
        .iter()
        .map(|name| sketch.number(name, 0, 29, &[0]))
        .collect();
    for (name, axis) in ["moveX", "moveY", "moveZ"].iter().zip(&axes) {
        sketch.action(name, below(axis, 29), &[(axis, plus(axis, 1))]);
    }
    sketch.rule("notThere", not(&is(&axes[0], 29)));
    sketch.model()
}

fn single_thread() -> Limits {
    Limits {
        keep_graph: true,
        ..Limits::default()
    }
}

#[test]
fn threads_that_become_idle_while_a_level_runs_change_nothing_and_come_back() {
    let model = cube();
    let expected = explore(&model, &single_thread());
    for idle_at_the_start in [0, 2] {
        let helpers = Helpers::new(idle_at_the_start);
        let outcome = thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..3 {
                    thread::sleep(Duration::from_millis(3));
                    helpers.add(1);
                }
            });
            explore(
                &model,
                &Limits {
                    helpers: Some(helpers.clone()),
                    ..single_thread()
                },
            )
        });
        assert_eq!(outcome, expected, "{idle_at_the_start} idle at the start");
        assert_eq!(
            helpers.idle(),
            idle_at_the_start + 3,
            "every thread is back when the search is done"
        );
    }
}
