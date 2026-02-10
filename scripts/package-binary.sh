#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 ]]; then
  echo "Usage: $0 <target> <artifact> <binary-name>" >&2
  exit 1
fi

target="$1"
artifact="$2"
binary_name="$3"

mkdir -p dist
cp "target/${target}/release/${binary_name}" "dist/${binary_name}"
tar czf "dist/${artifact}.tar.gz" -C dist "${binary_name}"
