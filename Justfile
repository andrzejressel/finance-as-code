set windows-shell := ["pwsh.exe", "-c"]
# renovate: datasource=crate depName=cargo-nextest packageName=cargo-nextest
NEXTEST_VERSION := "0.9.72"
# renovate: datasource=crate depName=sd packageName=sd
SD_VERSION := "1.0.0"
# renovate: datasource=crate depName=cargo-llvm-cov packageName=cargo-llvm-cov
CARGO_LLVM_COV_VERSION := "0.8.0"

install-requirements:
    rustup component add rustfmt
    rustup component add llvm-tools-preview
    cargo binstall --no-confirm cargo-nextest@{{NEXTEST_VERSION}}
    cargo binstall --no-confirm sd@{{SD_VERSION}}
    cargo binstall --no-confirm cargo-llvm-cov@{{CARGO_LLVM_COV_VERSION}}

release:
    cargo build --release

check:
    cargo fmt -- --check
    cargo clippy --tests

fmt:
    cargo fmt
    cargo clippy --tests --fix --allow-dirty --allow-staged

test:
    cargo nextest run

test-coverage:
    cargo llvm-cov nextest --cobertura --output-path covertura.xml

update-test-snapshots:
    cargo insta test --workspace --accept --test-runner nextest
