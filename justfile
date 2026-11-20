default: check

# One recipe per check. To add a check, write its recipe and name it here.
check: format unused-dependencies dependencies lint typescript test verify

format:
    cargo fmt --check

lint:
    cargo clippy --all-targets --locked -- -D warnings

dependencies:
    cargo deny check

unused-dependencies:
    cargo machete

typescript:
    pnpm --dir npm/model install --frozen-lockfile
    pnpm --dir npm/model run typecheck
    pnpm --dir npm/model run format
    pnpm --dir npm/model test

test:
    cargo test --locked

tlc:
    cargo build --release --locked -p earnest-cli
    for model in tasks/conformance/models/*.model.ts tasks/conformance/models/*.rs; do target/release/earnest confirm --tlc target/tools/tla2tools.jar "$model" || exit 1; done
    cargo clippy --locked -p earnest_break_it --features tlc --test tlc -- -D warnings
    cargo test --locked -p earnest_break_it --features tlc --test tlc

verify:
    cargo verus focus

mutants:
    cargo mutants --jobs {{ num_cpus() }} --timeout 30 --build-timeout 300

fix:
    cargo fmt
    cargo clippy --all-targets --locked --fix --allow-dirty --allow-staged
