set shell := ["powershell.exe", "-c"]

default: lint test

release:
  cargo build --release

lint:
  cargo clippy --workspace --all-targets --all-features

run:
  cargo run -p FinanseMonorepo-gui

bin name *args:
  cargo run --bin {{name}} -- {{args}}

example name *args:
  cargo run --example {{name}} -- {{args}}
  
test:
  cargo nextest run

test-project project:
  cargo nextest run -p {{project}}
    
update-test-snapshots:
  cargo insta test --workspace --accept --test-runner nextest
    
net-gui:
  cargo hot --features debug --bin gui -p spectra_net_app