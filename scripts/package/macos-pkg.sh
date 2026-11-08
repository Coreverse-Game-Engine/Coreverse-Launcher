#!/usr/bin/env bash
# Wraps the built .app into an installer package (Tauri has no .pkg target).
#
#   macos-pkg.sh --app "<path>/Coreverse Launcher.app" --version X.Y.Z --out <file.pkg>
#
# Optional environment: APPLE_INSTALLER_IDENTITY ("Developer ID Installer: ...") signs the
# package; with APPLE_ID, APPLE_PASSWORD and APPLE_TEAM_ID it is also notarized and stapled.
# Without them the package is built unsigned.
set -euo pipefail

die() {
    echo "macos-pkg: $*" >&2
    exit 1
}

app=""
version=""
out=""
while [ $# -gt 0 ]; do
    case "$1" in
        --app) app=${2:?--app needs a value}; shift 2 ;;
        --version) version=${2:?--version needs a value}; shift 2 ;;
        --out) out=${2:?--out needs a value}; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done
[ -d "$app" ] || die "--app must be an existing .app folder"
[ -n "$version" ] && [ -n "$out" ] || die "--version and --out are required"
mkdir -p "$(dirname "$out")"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/root/Applications"
ditto "$app" "$stage/root/Applications/$(basename "$app")"

# Without this the installer may "update" a copy of the app that lives somewhere else.
pkgbuild --analyze --root "$stage/root" "$stage/components.plist" >/dev/null
count=$(/usr/libexec/PlistBuddy -c "Print" "$stage/components.plist" | grep -c "BundleIsRelocatable" || true)
index=0
while [ "$index" -lt "$count" ]; do
    /usr/libexec/PlistBuddy -c "Set :$index:BundleIsRelocatable false" "$stage/components.plist"
    index=$((index + 1))
done

sign_args=()
if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ]; then
    sign_args=(--sign "$APPLE_INSTALLER_IDENTITY")
else
    echo "macos-pkg: APPLE_INSTALLER_IDENTITY is not set, the package will be unsigned" >&2
fi

pkgbuild \
    --root "$stage/root" \
    --component-plist "$stage/components.plist" \
    --identifier "com.coreverse.launcher.pkg" \
    --version "$version" \
    --install-location / \
    ${sign_args[@]+"${sign_args[@]}"} \
    "$out"

if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ] && [ -n "${APPLE_ID:-}" ] &&
    [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
    xcrun notarytool submit "$out" \
        --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait
    xcrun stapler staple "$out"
else
    echo "macos-pkg: not notarizing (a signed package and the APPLE_ID, APPLE_PASSWORD and APPLE_TEAM_ID variables are needed)" >&2
fi
echo "macos-pkg: wrote $out" >&2
