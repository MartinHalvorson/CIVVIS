#!/usr/bin/env bash
# GitHub's Ubuntu images route APT through a mirror list that can prefer
# azure.archive.ubuntu.com.  When that mirror stalls, a plain apt-get update
# consumes the whole short control-mod gate.  Prefer Canonical's archive and
# keep each network operation bounded so a transient outage can be retried.
set -euo pipefail

if (( $# == 0 )); then
  echo "usage: $0 PACKAGE [PACKAGE ...]" >&2
  exit 64
fi

readonly max_attempts=3
readonly apt_timeout_seconds=30
apt_options=(
  -o Acquire::Retries=0
  -o Acquire::http::Timeout=10
  -o Acquire::https::Timeout=10
)

prefer_canonical_ubuntu_archive() {
  local mirror_list=/etc/apt/apt-mirrors.txt

  # Ubuntu's mirror+file configuration reads this list at update time.  Keep
  # its URL scheme and paths intact; only remove the flaky Azure hostname.
  if [[ -f "$mirror_list" ]] && sudo grep -Fq 'azure.archive.ubuntu.com' "$mirror_list"; then
    echo "Using archive.ubuntu.com instead of azure.archive.ubuntu.com."
    sudo sed -i 's/azure\.archive\.ubuntu\.com/archive.ubuntu.com/g' "$mirror_list"
  fi
}

run_apt() {
  local subcommand=$1
  shift

  # Run timeout under sudo so it directly owns (and can kill) apt-get rather
  # than only timing out a sudo wrapper.
  sudo timeout --kill-after=5s "${apt_timeout_seconds}s" \
    env DEBIAN_FRONTEND=noninteractive apt-get \
      "${apt_options[@]}" "$subcommand" "$@"
}

update_apt_indexes() {
  # A mirror list can report one dead URL even after another URL has supplied
  # the complete index.  `--error-on=any` turns that harmless fallback into a
  # failed update, which then makes this bounded installer retry the same
  # usable mirror three times.  Retry once without the strict aggregate
  # verdict; `apt-get install` still verifies that the requested package is
  # actually available from the indexes that were downloaded.
  if run_apt update --error-on=any; then
    return 0
  fi

  echo "APT update saw a mirror error; accepting usable fallback indexes." >&2
  run_apt update
}

prefer_canonical_ubuntu_archive

for ((attempt = 1; attempt <= max_attempts; attempt++)); do
  echo "APT install attempt ${attempt}/${max_attempts}: $*"
  if update_apt_indexes && \
      run_apt install --no-install-recommends -y "$@"; then
    exit 0
  fi

  if (( attempt < max_attempts )); then
    echo "APT attempt ${attempt}/${max_attempts} failed; retrying shortly." >&2
    sleep "$attempt"
  fi
done

echo "APT could not install after ${max_attempts} bounded attempts: $*" >&2
exit 1
