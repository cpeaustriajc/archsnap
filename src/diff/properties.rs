use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;

use super::{NO_CHANGES, diff};
use crate::scan::{Edge, Link, Module, Snapshot, Stats};
use crate::system::System;

fn name() -> impl Strategy<Value = String> {
    "[a-z]{1,4}(/[a-z]{1,4}){0,2}"
}

fn module() -> impl Strategy<Value = Module> {
    (0u32..5000, 0u32..100_000, "[a-z]{0,4}").prop_map(|(files, lines, package)| Module {
        files,
        lines,
        package,
    })
}

fn link() -> impl Strategy<Value = Link> {
    (name(), name(), vec("[a-z]{1,4}", 0..3)).prop_map(|(from, to, names)| Link { from, to, names })
}

fn edge() -> impl Strategy<Value = Edge> {
    (name(), name(), 0u32..50, vec(link(), 0..3)).prop_map(|(from, to, count, links)| Edge {
        from,
        to,
        count,
        links,
    })
}

fn stats() -> impl Strategy<Value = Stats> {
    (0u32..5000, 0u32..100_000, 0u32..50, 0u32..50, 0u32..50).prop_map(
        |(files, lines, skipped, ignored, unresolved)| Stats {
            files,
            lines,
            skipped,
            ignored,
            unresolved,
        },
    )
}

fn snapshot() -> impl Strategy<Value = Snapshot> {
    (
        (0u32..10, "[0-9]{4}-W[0-9]{2}", "[0-9a-f]{7}", 1usize..5),
        btree_map(name(), module(), 0..8),
        vec(edge(), 0..8),
        btree_map("[a-z@/]{1,8}", 0u32..100, 0..6),
        stats(),
    )
        .prop_map(
            |((schema, week, sha, depth), modules, edges, externals, stats)| Snapshot {
                schema,
                tool: String::new(),
                week,
                sha,
                date: "2026-01-05".to_string(),
                depth,
                modules,
                edges,
                externals,
                stats,
                system: System::default(),
            },
        )
}

#[test]
fn snapshot_compared_with_itself_has_no_changes() {
    TestRunner::default()
        .run(&snapshot(), |current| {
            let result = diff(Some(&current), &current);
            prop_assert!(result.added_modules.is_empty());
            prop_assert!(result.removed_modules.is_empty());
            prop_assert!(result.grown.is_empty());
            prop_assert!(result.shrunk.is_empty());
            prop_assert!(result.added_edges.is_empty());
            prop_assert!(result.removed_edges.is_empty());
            prop_assert!(result.added_packages.is_empty());
            prop_assert!(result.removed_packages.is_empty());
            prop_assert_eq!(result.summary, vec![NO_CHANGES.to_string()]);
            Ok(())
        })
        .unwrap();
}
