set shell := ["powershell.exe", "-c"]

release:
    cargo build --release

check:
    cargo fmt -- --check
    cargo clippy --tests --all-features

fmt:
    cargo fmt
    cargo clippy --tests --all-features --fix --allow-dirty --allow-staged

test:
    cargo nextest run

update-test-snapshots:
    cargo insta test --workspace --accept --test-runner nextest
