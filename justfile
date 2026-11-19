default: check

# One recipe per check. To add a check, write its recipe and name it here.
check: format lint source-rules dependencies unused-dependencies test

format:
    cargo fmt --check

lint:
    cargo clippy --all-targets --locked -- -D warnings

# Every rule file under rules/ runs; a new rule needs no change here.
source-rules:
    ast-grep test --skip-snapshot-tests
    ast-grep scan

dependencies:
    cargo deny check

unused-dependencies:
    cargo machete

test:
    cargo test --locked

fix:
    cargo fmt
    cargo clippy --all-targets --locked --fix --allow-dirty --allow-staged
