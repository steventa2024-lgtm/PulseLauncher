#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo test -p pulslauncher-core
npm run typecheck
npm test
npm run lint
npm run build
cargo clippy --workspace --all-targets -- -D warnings
echo "Linux verification finished. This script does not certify Windows."
