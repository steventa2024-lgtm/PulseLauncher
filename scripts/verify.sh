#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo test --workspace
npm test
npm run lint
npx tsc --noEmit
