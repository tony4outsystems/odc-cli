#!/usr/bin/env bash
# Drives the fake `odc` binary in scripts/demo/odc to produce a realistic,
# reproducible terminal session for asciinema recording.
set -euo pipefail

cd "$(dirname "$0")"
export PATH="$PWD:$PATH"
PS1='$ '

type_cmd() {
  local cmd="$1"
  for ((i = 0; i < ${#cmd}; i++)); do
    printf '%s' "${cmd:$i:1}"
    sleep 0.02
  done
  printf '\n'
}

clear
printf '$ '
type_cmd "odc list-apps --search eGov"
odc list-apps --search eGov
sleep 1.5

printf '$ '
type_cmd "odc validate --asset eGovPortal --env Production"
odc validate --asset eGovPortal --env Production
sleep 1.8

printf '$ '
type_cmd "odc deploy --asset eGovPortal --env Production"
odc deploy --asset eGovPortal --env Production
sleep 2.5
