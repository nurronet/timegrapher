#!/usr/bin/env bash
# Install Ubuntu packages for a CI job, trying again when the package
# mirror can't be reached. A runner whose DNS or mirror hiccups (#1467)
# otherwise fails the job before any test runs.
# Usage: .github/ci/apt-install.sh <package>...
set -u
for attempt in 1 2 3; do
  if sudo apt-get update && sudo apt-get install -y "$@"; then
    exit 0
  fi
  if [[ $attempt -lt 3 ]]; then
    echo "apt-get attempt $attempt failed; trying again in 30 s"
    sleep 30
  fi
done
echo "apt-get failed 3 times"
exit 1
