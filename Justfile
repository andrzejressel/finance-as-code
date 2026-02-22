set windows-shell := ["pwsh.exe", "-c"]

install:
    rustup component add rustfmt
    rustup +nightly component add rustfmt
    rustup component add clippy
    rustup component add llvm-tools-preview
    mise install

release:
    cargo build --release

check:
    cargo +nightly fmt -- --check
    cargo clippy --tests

fmt:
    cargo +nightly fmt
    cargo clippy --tests --fix --allow-dirty --allow-staged

test:
    cargo nextest run
    cargo test --doc --features all

test-coverage:
    cargo llvm-cov nextest --cobertura --output-path covertura.xml

update-test-snapshots:
    cargo insta test --workspace --accept --test-runner nextest

docs $RUSTDOCFLAGS="--cfg docsrs":
    cargo +nightly doc --no-deps --features all -p finance_as_code_budget
