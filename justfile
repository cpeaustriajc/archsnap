default: check

# One recipe per check. To add a check, write its recipe and name it here.
check: format unused-dependencies dependencies lint test verify

format:
    cargo fmt --check

lint:
    cargo clippy --all-targets --locked -- -D warnings

dependencies:
    cargo deny check

unused-dependencies:
    cargo machete

test:
    cargo test --locked

verify:
    cargo verus focus

mutants:
    cargo mutants --jobs {{ num_cpus() }} --timeout 30 --build-timeout 300

fix:
    cargo fmt
    cargo clippy --all-targets --locked --fix --allow-dirty --allow-staged
