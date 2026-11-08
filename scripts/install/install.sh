#!/bin/sh
# Installs Coreverse Launcher on Linux from the latest GitHub release.
#
#   curl -fsSL https://github.com/Coreverse-Game-Engine/Coreverse-Launcher/releases/latest/download/install.sh | sh
#   sh install.sh [--version X.Y.Z] [--format auto|deb|rpm|pacman|appimage] [--dry-run]
#
# It picks the package that fits the distribution (Debian and Ubuntu: .deb, Fedora and
# openSUSE: .rpm, Arch and CachyOS: pacman package, anything else: AppImage), checks the
# download against the release's SHA256SUMS and installs it with the system package manager.
set -eu

REPO="Coreverse-Game-Engine/Coreverse-Launcher"
PRODUCT="Coreverse-Launcher"

version=""
format="auto"
dry_run=0

die() {
    echo "install.sh: $*" >&2
    exit 1
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version) [ $# -ge 2 ] || die "--version needs a value"; version=${2#v}; shift 2 ;;
        --format) [ $# -ge 2 ] || die "--format needs a value"; format=$2; shift 2 ;;
        --dry-run) dry_run=1; shift ;;
        -h | --help) sed -n '2,9p' "$0"; exit 0 ;;
        *) die "unknown argument: $1" ;;
    esac
done

case "$(uname -s)" in
    Linux) ;;
    Darwin) die "on macOS use: brew install --cask coreverse-launcher (or download the .dmg)" ;;
    *) die "on Windows use: winget install Coreverse.Launcher (or scoop)" ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=arm64 ;;
    *) die "unsupported CPU architecture: $(uname -m)" ;;
esac

have() { command -v "$1" >/dev/null 2>&1; }

fetch() { # url, file
    if have curl; then
        curl -fsSL --retry 3 -o "$2" "$1"
    elif have wget; then
        wget -q -O "$2" "$1"
    else
        die "curl or wget is required"
    fi
}

# Which family of distributions this is, from /etc/os-release.
detect_family() {
    release_file=${COREVERSE_OS_RELEASE:-/etc/os-release}
    [ -r "$release_file" ] || { echo other; return; }
    ids=$(. "$release_file" && echo "${ID:-} ${ID_LIKE:-}")
    for id in $ids; do
        case "$id" in
            debian | ubuntu) echo deb; return ;;
            fedora) echo rpm-dnf; return ;;
            suse | opensuse | opensuse-leap | opensuse-tumbleweed | sles) echo rpm-zypper; return ;;
            arch) echo pacman; return ;;
        esac
    done
    echo other
}

if [ "$format" = auto ]; then
    case "$(detect_family)" in
        deb) format=deb ;;
        rpm-dnf | rpm-zypper) format=rpm ;;
        pacman) format=pacman ;;
        *) format=appimage ;;
    esac
fi
case "$format" in
    deb) ext=deb ;;
    rpm) ext=rpm ;;
    pacman) ext=pkg.tar.zst ;;
    appimage) ext=AppImage ;;
    *) die "unknown format: $format" ;;
esac

if [ -z "$version" ]; then
    if [ "$dry_run" -eq 1 ] && ! have curl && ! have wget; then
        version="latest"
    elif have curl; then
        final=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest") ||
            die "could not look up the latest release"
        version=${final##*/v}
    else
        version=$(wget -qO- "https://api.github.com/repos/$REPO/releases/latest" |
            sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p' | head -n 1)
    fi
    [ -n "$version" ] || die "could not find the latest release"
fi

file="${PRODUCT}_${version}_linux_${arch}.${ext}"
base="https://github.com/$REPO/releases/download/v${version}"

as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif have sudo; then
        sudo "$@"
    else
        die "root rights are needed: run as root or install sudo"
    fi
}

install_command() { # path
    case "$format" in
        deb) echo "apt-get install -y $1" ;;
        rpm)
            if [ "$(detect_family)" = rpm-zypper ]; then
                echo "zypper --non-interactive --no-gpg-checks install $1"
            else
                echo "dnf install -y $1"
            fi
            ;;
        pacman) echo "pacman -U --noconfirm $1" ;;
    esac
}

if [ "$dry_run" -eq 1 ]; then
    echo "Would download $base/$file"
    echo "Would verify it against $base/SHA256SUMS"
    if [ "$format" = appimage ]; then
        echo "Would install it to \$HOME/.local/bin/coreverse-launcher"
    else
        echo "Would run (as root): $(install_command "./$file")"
    fi
    exit 0
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

echo "Downloading $file ..."
fetch "$base/$file" "$tmp/$file"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"

expected=$(grep "  $file\$" "$tmp/SHA256SUMS" | cut -d ' ' -f 1)
[ -n "$expected" ] || die "$file is not listed in the release's SHA256SUMS"
if have sha256sum; then
    actual=$(sha256sum "$tmp/$file" | cut -d ' ' -f 1)
elif have shasum; then
    actual=$(shasum -a 256 "$tmp/$file" | cut -d ' ' -f 1)
else
    die "sha256sum is required to verify the download"
fi
[ "$expected" = "$actual" ] || die "the download does not match its checksum, nothing was installed"
echo "Checksum verified."

if [ "$format" = appimage ]; then
    target="${HOME}/.local/bin"
    mkdir -p "$target"
    install -m 755 "$tmp/$file" "$target/coreverse-launcher"
    echo "Installed to $target/coreverse-launcher (needs FUSE 2, or run it with --appimage-extract-and-run)."
    case ":$PATH:" in *":$target:"*) ;; *) echo "Add $target to your PATH to start it by name." ;; esac
else
    # The package manager needs a path it can read after the temporary folder is protected.
    chmod 644 "$tmp/$file"
    # shellcheck disable=SC2046  # the command is built from fixed words
    as_root $(install_command "$tmp/$file")
fi
echo "Coreverse Launcher $version is installed."
