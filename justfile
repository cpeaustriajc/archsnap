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
    pnpm --dir npm/cli install --frozen-lockfile
    pnpm --dir npm/cli run typecheck
    pnpm --dir npm/cli run format
    pnpm --dir npm/cli test

test:
    cargo test --locked

tlc:
    cargo build --release --locked -p earnest-cli
    for model in tasks/conformance/models/*.model.ts tasks/conformance/models/*.rs tasks/conformance/twins/*.model.ts tasks/conformance/twins/*.rs; do target/release/earnest confirm --tlc target/tools/tla2tools.jar "$model" || exit 1; done
    cargo clippy --locked -p earnest_break_it --features tlc --test tlc -- -D warnings
    cargo test --locked -p earnest_break_it --features tlc --test tlc

tla-rs:
    cargo install tla-tools --locked --version 0.3.0 --root target/tools
    cargo build --release --locked -p earnest-cli
    for model in tasks/conformance/models/*.model.ts tasks/conformance/models/*.rs tasks/conformance/twins/*.model.ts tasks/conformance/twins/*.rs; do target/release/earnest confirm --tla-rs target/tools/bin/tla-rs "$model" || exit 1; done
    cargo clippy --locked -p earnest_break_it --features tla_rs --test tla_rs -- -D warnings
    cargo test --locked -p earnest_break_it --features tla_rs --test tla_rs

benchmark:
    cargo build --release --locked -p earnest-cli
    cargo run --release --locked -p earnest_benchmark -- --runs 5

verify:
    cargo verus focus

mutants:
    cargo mutants --jobs {{ num_cpus() }} --timeout 30 --build-timeout 300

fix:
    cargo fmt
    cargo clippy --all-targets --locked --fix --allow-dirty --allow-staged
