#!/bin/sh
set -eu
: "${INFRAI_API_KEY:?set INFRAI_API_KEY before running the worker}"
cargo run --bin appointment-queue-worker
