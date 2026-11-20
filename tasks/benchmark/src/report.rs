//! Medians, rates and the Markdown the benchmark prints.

use std::time::Duration;

const KIB_PER_MIB: f64 = 1024.0;
pub(crate) const TOGETHER_TARGET_SECONDS: u32 = 5;
pub(crate) const SLOWEST_TARGET_SECONDS: u32 = 10;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) model: String,
    pub(crate) language: String,
    pub(crate) states: u64,
    pub(crate) median: Duration,
    pub(crate) peak_kib: Option<u64>,
}

pub(crate) fn median(times: &[Duration]) -> Duration {
    let mut sorted = times.to_vec();
    sorted.sort();
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => Duration::ZERO,
        length if length % 2 == 1 => sorted[middle],
        _ => (sorted[middle - 1] + sorted[middle]) / 2,
    }
}

fn float(number: u64) -> f64 {
    f64::from(u32::try_from(number).unwrap_or(u32::MAX))
}

pub(crate) fn states_per_second(states: u64, wall: Duration) -> u64 {
    let micros = wall.as_micros().max(1);
    u64::try_from(u128::from(states) * 1_000_000 / micros).unwrap_or(u64::MAX)
}

fn seconds(wall: Duration) -> String {
    format!("{:.3}", wall.as_secs_f64())
}

pub(crate) fn with_separators(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

pub(crate) fn memory(peak_kib: Option<u64>) -> String {
    peak_kib.map_or_else(
        || "n/a".to_owned(),
        |kib| format!("{:.1}", float(kib) / KIB_PER_MIB),
    )
}

pub(crate) fn table(rows: &[Row]) -> String {
    let header = [
        "| Model | Language | States | Median wall time (s) | States per second | Peak memory (MiB) |",
        "|---|---|---:|---:|---:|---:|",
    ]
    .map(str::to_owned);
    let body = rows.iter().map(|row| {
        format!(
            "| {} | {} | {} | {} | {} | {} |",
            row.model,
            row.language,
            with_separators(row.states),
            seconds(row.median),
            with_separators(states_per_second(row.states, row.median)),
            memory(row.peak_kib)
        )
    });
    let mut text = header
        .into_iter()
        .chain(body)
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}

pub(crate) fn spec_lines(together: Duration, slowest: Duration) -> [String; 2] {
    [
        format!(
            "12 models together: {} s, target under {TOGETHER_TARGET_SECONDS} s (a stand-in: these benchmark models, not the first app's 12)",
            seconds(together)
        ),
        format!(
            "slowest model alone: {} s, target under {SLOWEST_TARGET_SECONDS} s (a stand-in: these benchmark models, not the first app's)",
            seconds(slowest)
        ),
    ]
}

pub(crate) fn speedup_line(model: &str, one: Duration, all: Duration, cores: usize) -> String {
    let speedup = one.as_secs_f64() / all.as_secs_f64().max(f64::EPSILON);
    format!(
        "{model}: 1 worker {} s, {cores} workers (all cores) {} s, speed-up {speedup:.2}x",
        seconds(one),
        seconds(all)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn millis(count: u64) -> Duration {
        Duration::from_millis(count)
    }

    #[test]
    fn median_of_odd_even_and_no_runs() {
        assert_eq!(
            median(&[millis(30), millis(10), millis(20)]),
            millis(20),
            "odd count"
        );
        assert_eq!(
            median(&[millis(40), millis(10), millis(20), millis(30)]),
            millis(25),
            "even count"
        );
        assert_eq!(median(&[]), Duration::ZERO, "no runs");
    }

    #[test]
    fn rate_is_states_over_seconds() {
        assert_eq!(
            states_per_second(1_000_000, millis(2_000)),
            500_000,
            "a million states in two seconds"
        );
        assert_eq!(
            states_per_second(5, Duration::ZERO),
            5_000_000,
            "zero time does not divide by zero"
        );
    }

    #[test]
    fn thousands_are_separated() {
        assert_eq!(with_separators(0), "0", "zero");
        assert_eq!(with_separators(999), "999", "below a thousand");
        assert_eq!(with_separators(1_000), "1,000", "a thousand");
        assert_eq!(with_separators(1_234_567), "1,234,567", "millions");
    }

    #[test]
    fn table_has_a_header_and_one_line_per_row() {
        let rows = [
            Row {
                model: "Grid1k".to_owned(),
                language: "Rust".to_owned(),
                states: 1_000,
                median: millis(500),
                peak_kib: Some(2_048),
            },
            Row {
                model: "Other".to_owned(),
                language: "TypeScript".to_owned(),
                states: 10,
                median: millis(1_500),
                peak_kib: None,
            },
        ];
        let text = table(&rows);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "header, rule and two rows");
        assert_eq!(
            lines[2], "| Grid1k | Rust | 1,000 | 0.500 | 2,000 | 2.0 |",
            "first row"
        );
        assert_eq!(
            lines[3], "| Other | TypeScript | 10 | 1.500 | 6 | n/a |",
            "second row"
        );
    }

    #[test]
    fn spec_lines_name_the_targets_and_the_stand_in() {
        let [together, slowest] = spec_lines(millis(1_250), millis(2_500));
        assert!(
            together.starts_with("12 models together: 1.250 s, target under 5 s"),
            "{together}"
        );
        assert!(
            slowest.starts_with("slowest model alone: 2.500 s, target under 10 s"),
            "{slowest}"
        );
        assert!(
            together.contains("stand-in") && slowest.contains("stand-in"),
            "both are marked"
        );
    }

    #[test]
    fn speedup_is_one_worker_over_all_cores() {
        assert_eq!(
            speedup_line("Big", millis(8_000), millis(2_000), 4),
            "Big: 1 worker 8.000 s, 4 workers (all cores) 2.000 s, speed-up 4.00x",
            "four times faster"
        );
    }
}
