#!/usr/bin/env bash
# Imports the Apple signing certificates into a temporary keychain and exports the signing
# variables to the rest of the GitHub Actions job. Without APPLE_CERTIFICATE it does nothing,
# and the build stays unsigned.
#
# Environment: APPLE_CERTIFICATE and APPLE_CERTIFICATE_PASSWORD (base64 .p12 of the
# "Developer ID Application" certificate), optionally APPLE_INSTALLER_CERTIFICATE (base64 .p12
# of "Developer ID Installer", same password) and APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID for
# notarization.
set -euo pipefail

env_file=${GITHUB_ENV:-/dev/null}
export_var() {
    if [ -n "${2:-}" ]; then
        printf '%s=%s\n' "$1" "$2" >>"$env_file"
    fi
}

if [ -z "${APPLE_CERTIFICATE:-}" ]; then
    echo "macos-keychain: no APPLE_CERTIFICATE, building unsigned"
    exit 0
fi

temp=${RUNNER_TEMP:-$(mktemp -d)}
keychain="$temp/coreverse-build.keychain-db"
password=$(uuidgen)
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"

import_p12() {
    local file="$temp/certificate-$1.p12"
    printf '%s' "$2" | base64 --decode >"$file"
    security import "$file" -k "$keychain" -P "${APPLE_CERTIFICATE_PASSWORD:-}" \
        -T /usr/bin/codesign -T /usr/bin/productsign -T /usr/bin/pkgbuild
    rm -f "$file"
}
import_p12 application "$APPLE_CERTIFICATE"
if [ -n "${APPLE_INSTALLER_CERTIFICATE:-}" ]; then
    import_p12 installer "$APPLE_INSTALLER_CERTIFICATE"
fi

# Make the new keychain visible without dropping the existing ones.
existing=$(security list-keychains -d user | tr -d '"' | tr '\n' ' ')
# shellcheck disable=SC2086  # a list of keychain paths
security list-keychains -d user -s "$keychain" $existing
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$password" "$keychain" >/dev/null

identity() {
    security find-identity -v "$keychain" | sed -n "s/.*\"\\($1:.*\\)\"/\\1/p" | head -n 1
}
application=$(identity "Developer ID Application")
installer=$(identity "Developer ID Installer")
[ -n "$application" ] || {
    echo "macos-keychain: no Developer ID Application identity found" >&2
    exit 1
}
export_var APPLE_SIGNING_IDENTITY "$application"
export_var APPLE_INSTALLER_IDENTITY "$installer"
export_var APPLE_ID "${APPLE_ID:-}"
export_var APPLE_PASSWORD "${APPLE_PASSWORD:-}"
export_var APPLE_TEAM_ID "${APPLE_TEAM_ID:-}"
echo "macos-keychain: signing as $application"
