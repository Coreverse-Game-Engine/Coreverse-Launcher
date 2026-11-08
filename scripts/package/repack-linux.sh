#!/usr/bin/env bash
# Builds an .rpm or a pacman package from the .deb that `tauri build` produced, so all Linux
# packages share one file layout (binary, desktop entry, icons, URL scheme handler).
#
#   repack-linux.sh --deb <file.deb> --format rpm|pacman --out <dir>
#
# The result is named Coreverse-Launcher_<version>_linux_<x86_64|arm64>.<rpm|pkg.tar.zst>.
# Needs: ar, tar and rpmbuild (rpm) or zstd (pacman).
set -euo pipefail

die() {
    echo "repack-linux: $*" >&2
    exit 1
}

deb=""
format=""
out="."
while [ $# -gt 0 ]; do
    case "$1" in
        --deb) deb=${2:?--deb needs a value}; shift 2 ;;
        --format) format=${2:?--format needs a value}; shift 2 ;;
        --out) out=${2:?--out needs a value}; shift 2 ;;
        -h | --help) sed -n '2,8p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done
[ -f "$deb" ] || die "--deb must be an existing file"
case "$format" in rpm | pacman) ;; *) die "--format must be rpm or pacman" ;; esac
command -v ar >/dev/null || die "ar is required"
mkdir -p "$out"
deb=$(cd "$(dirname "$deb")" && pwd)/$(basename "$deb")
out=$(cd "$out" && pwd)

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/deb" "$work/root" "$work/control"

(cd "$work/deb" && ar x "$deb")
control_tar=$(find "$work/deb" -name 'control.tar*' | head -n 1)
data_tar=$(find "$work/deb" -name 'data.tar*' | head -n 1)
[ -n "$control_tar" ] && [ -n "$data_tar" ] || die "$deb is not a Debian package"
tar -xf "$control_tar" -C "$work/control"
tar -xf "$data_tar" -C "$work/root"

field() { sed -n "s/^$1: //p" "$work/control/control" | head -n 1; }
version=$(field Version)
deb_arch=$(field Architecture)
summary=$(field Description)
[ -n "$version" ] || die "the package has no version"
[ -x "$work/root/usr/bin/coreverse-launcher" ] || die "usr/bin/coreverse-launcher is missing"

case "$deb_arch" in
    amd64) asset_arch=x86_64; native_arch=x86_64 ;;
    arm64) asset_arch=arm64; native_arch=aarch64 ;;
    *) die "unsupported architecture: $deb_arch" ;;
esac
url="https://github.com/Coreverse-Game-Engine/Coreverse-Launcher"
license="GPL-3.0-only"

build_rpm() {
    command -v rpmbuild >/dev/null || die "rpmbuild is required (Debian: apt install rpm)"
    local top="$work/rpm" spec
    mkdir -p "$top/SPECS" "$top/RPMS" "$top/BUILD" "$top/BUILDROOT"
    spec="$top/SPECS/coreverse-launcher.spec"

    cat >"$spec" <<SPEC
%global debug_package %{nil}
%global __strip /bin/true
%global __os_install_post %{nil}
%define _build_id_links none
%define _unpackaged_files_terminate_build 0
AutoReqProv: no

Name: coreverse-launcher
Version: $version
Release: 1
Summary: $summary
License: $license
URL: $url
# The same capabilities exist on Fedora and openSUSE, whatever the package names are.
Requires: libwebkit2gtk-4.1.so.0()(64bit)
Requires: libgtk-3.so.0()(64bit)
Requires: libdbus-1.so.3()(64bit)
Suggests: gnome-keyring

%description
The official launcher of the Coreverse ecosystem. It installs and manages Coreverse Engine
versions and signs you in to your Coreverse account. A Secret Service provider such as
GNOME Keyring or KWallet keeps you signed in between launches.

%install
mkdir -p %{buildroot}
cp -a "%{stage}/." %{buildroot}/

%post
/usr/bin/update-desktop-database -q /usr/share/applications >/dev/null 2>&1 || :
/usr/bin/gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor >/dev/null 2>&1 || :

%postun
/usr/bin/update-desktop-database -q /usr/share/applications >/dev/null 2>&1 || :
/usr/bin/gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor >/dev/null 2>&1 || :

%files
%defattr(-,root,root,-)
SPEC
    (cd "$work/root" && find . \( -type f -o -type l \) | sort | sed 's|^\.|"|; s|$|"|') >>"$spec"

    rpmbuild -bb \
        --define "_topdir $top" \
        --define "stage $work/root" \
        --define "_binary_payload w9.gzdio" \
        --define "_buildhost coreverse" \
        --target "$native_arch" \
        "$spec" >&2
    local built
    built=$(find "$top/RPMS" -name '*.rpm' | head -n 1)
    [ -n "$built" ] || die "rpmbuild produced no package"
    cp "$built" "$out/Coreverse-Launcher_${version}_linux_${asset_arch}.rpm"
}

build_pacman() {
    command -v zstd >/dev/null || die "zstd is required"
    local size entries
    size=$(du -sb "$work/root" | cut -f1)
    cat >"$work/root/.PKGINFO" <<PKGINFO
pkgname = coreverse-launcher
pkgbase = coreverse-launcher
pkgver = $version-1
pkgdesc = $summary
url = $url
builddate = ${SOURCE_DATE_EPOCH:-$(date +%s)}
packager = Coreverse
size = $size
arch = $native_arch
license = $license
depend = webkit2gtk-4.1
depend = gtk3
depend = dbus
optdepend = gnome-keyring: keeps you signed in between launches
optdepend = kwallet: keeps you signed in between launches
PKGINFO
    entries=$(cd "$work/root" && ls -A | grep -v '^\.PKGINFO$' | sort | tr '\n' ' ')
    # shellcheck disable=SC2086  # the entries are plain top-level folder names
    (cd "$work/root" && LC_ALL=C tar --owner=0 --group=0 --numeric-owner \
        -cf - .PKGINFO $entries | zstd -19 -T0 -q -o "$out/Coreverse-Launcher_${version}_linux_${asset_arch}.pkg.tar.zst")
}

if [ "$format" = rpm ]; then build_rpm; else build_pacman; fi
echo "repack-linux: wrote the $format package for $asset_arch ($version)" >&2
