// Renders the package manager files of a release: winget, Scoop, Homebrew and the AUR.
// Pure functions: they get the release facts and return file contents.

import { AUR_PACKAGE, APP_NAME, PRODUCT, REPO, WINGET_ID, assetName } from "./lib.mjs";

const HOMEPAGE = `https://github.com/${REPO}`;
const DESCRIPTION = "The official launcher of the Coreverse ecosystem";

function url(tag, name) {
    return `https://github.com/${REPO}/releases/download/${tag}/${name}`;
}

/**
 * @param {object} release `{ version, tag, date, assets }`; `assets` maps a file name to
 *   `{ sha256 }` and `msi` maps an architecture to `{ productCode, upgradeCode }`.
 */
export function wingetManifests({ version, tag, date, assets, msi = {} }) {
    const head = `PackageIdentifier: ${WINGET_ID}\nPackageVersion: ${version}\n`;
    const installers = [
        ["x64", "x86_64"],
        ["arm64", "arm64"],
    ].map(([wingetArch, arch]) => {
        const name = assetName(version, "windows", arch, "msi");
        const codes = msi[arch] ?? {};
        const lines = [
            `  - Architecture: ${wingetArch}`,
            `    InstallerUrl: ${url(tag, name)}`,
            `    InstallerSha256: ${assets[name].sha256.toUpperCase()}`,
        ];
        if (codes.productCode) {
            lines.push(`    ProductCode: "${codes.productCode}"`);
            lines.push("    AppsAndFeaturesEntries:");
            lines.push(`      - DisplayName: ${APP_NAME}`);
            lines.push("        Publisher: Coreverse");
            lines.push(`        DisplayVersion: ${version}`);
            lines.push(`        ProductCode: "${codes.productCode}"`);
            if (codes.upgradeCode) {
                lines.push(`        UpgradeCode: "${codes.upgradeCode}"`);
            }
        }
        return lines.join("\n");
    });

    return {
        [`${WINGET_ID}.yaml`]:
            `${head}DefaultLocale: en-US\nManifestType: version\nManifestVersion: 1.10.0\n`,
        [`${WINGET_ID}.installer.yaml`]:
            `${head}InstallerLocale: en-US\nMinimumOSVersion: 10.0.17763.0\n` +
            "InstallerType: wix\nScope: machine\nInstallModes:\n  - interactive\n  - silent\n" +
            `  - silentWithProgress\nUpgradeBehavior: install\nReleaseDate: ${date}\n` +
            `Installers:\n${installers.join("\n")}\nManifestType: installer\nManifestVersion: 1.10.0\n`,
        [`${WINGET_ID}.locale.en-US.yaml`]:
            `${head}PackageLocale: en-US\nPublisher: Coreverse\nPublisherUrl: ${HOMEPAGE}\n` +
            `PackageName: ${APP_NAME}\nPackageUrl: ${HOMEPAGE}\nLicense: GPL-3.0-only\n` +
            `ShortDescription: ${DESCRIPTION}\nMoniker: coreverse-launcher\nTags:\n  - coreverse\n` +
            `  - launcher\n  - game-engine\nReleaseNotesUrl: ${HOMEPAGE}/releases/tag/${tag}\n` +
            "ManifestType: defaultLocale\nManifestVersion: 1.10.0\n",
    };
}

export function scoopManifest({ version, tag, assets }) {
    const entry = (arch) => {
        const name = assetName(version, "windows", arch, "zip");
        return { url: url(tag, name), hash: assets[name].sha256 };
    };
    const base = `https://github.com/${REPO}/releases/download/v\${version}`;
    const updated = (arch) => ({
        url: `${base}/${assetName("${version}", "windows", arch, "zip")}`,
        hash: { url: "$baseurl/SHA256SUMS" },
    });
    const manifest = {
        version,
        description: DESCRIPTION,
        homepage: HOMEPAGE,
        license: "GPL-3.0-only",
        suggest: { "WebView2 Runtime": "extras/webview2" },
        architecture: { "64bit": entry("x86_64"), arm64: entry("arm64") },
        shortcuts: [["coreverse-launcher.exe", APP_NAME]],
        checkver: "github",
        autoupdate: { architecture: { "64bit": updated("x86_64"), arm64: updated("arm64") } },
    };
    return `${JSON.stringify(manifest, null, 4)}\n`;
}

export function homebrewCask({ version, assets }) {
    const sha = (arch) => assets[assetName(version, "macos", arch, "dmg")].sha256;
    const bundleId = "com.coreverse.launcher";
    return `cask "coreverse-launcher" do
  arch arm: "arm64", intel: "x86_64"

  version "${version}"
  sha256 arm:   "${sha("arm64")}",
         intel: "${sha("x86_64")}"

  url "${HOMEPAGE}/releases/download/v#{version}/${PRODUCT}_#{version}_macos_#{arch}.dmg"
  name "${APP_NAME}"
  desc "${DESCRIPTION}"
  homepage "${HOMEPAGE}"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on macos: ">= :big_sur"

  app "${APP_NAME}.app"

  zap trash: [
    "~/Library/Application Support/${bundleId}",
    "~/Library/Caches/${bundleId}",
    "~/Library/Logs/${bundleId}",
    "~/Library/Preferences/${bundleId}.plist",
    "~/Library/Saved Application State/${bundleId}.savedState",
    "~/Library/WebKit/${bundleId}",
  ]
end
`;
}

/** The AUR package repackages the release's .deb; returns `PKGBUILD` and `.SRCINFO`. */
export function aurFiles({ version, tag, assets }) {
    const debs = [
        ["x86_64", "x86_64"],
        ["aarch64", "arm64"],
    ].map(([archLinux, arch]) => {
        const name = assetName(version, "linux", arch, "deb");
        return { archLinux, name, sha256: assets[name].sha256 };
    });
    const common = {
        pkgdesc: DESCRIPTION,
        depends: ["webkit2gtk-4.1", "gtk3", "dbus"],
        optdepends: [
            "gnome-keyring: keeps you signed in between launches",
            "kwallet: keeps you signed in between launches",
        ],
    };
    const quoted = (list) => list.map((item) => `'${item}'`).join(" ");

    const pkgbuild = `# Maintainer: Coreverse <${HOMEPAGE}>
pkgname=${AUR_PACKAGE}
pkgver=${version}
pkgrel=1
pkgdesc='${common.pkgdesc}'
arch=('x86_64' 'aarch64')
url='${HOMEPAGE}'
license=('GPL-3.0-only')
depends=(${quoted(common.depends)})
optdepends=(${quoted(common.optdepends)})
provides=('coreverse-launcher')
conflicts=('coreverse-launcher')
options=('!strip' '!debug')
_release="${HOMEPAGE}/releases/download/${tag}"
${debs.map((deb) => `source_${deb.archLinux}=("$_release/${deb.name}")`).join("\n")}
${debs.map((deb) => `sha256sums_${deb.archLinux}=('${deb.sha256}')`).join("\n")}
noextract=(${debs.map((deb) => `'${deb.name}'`).join(" ")})

package() {
    local deb
    case "$CARCH" in
        aarch64) deb="${PRODUCT}_\${pkgver}_linux_arm64.deb" ;;
        *) deb="${PRODUCT}_\${pkgver}_linux_x86_64.deb" ;;
    esac
    bsdtar -xOf "$srcdir/$deb" 'data.tar.*' | bsdtar -xf - -C "$pkgdir"
}
`;

    const srcinfo = [
        `pkgbase = ${AUR_PACKAGE}`,
        `\tpkgdesc = ${common.pkgdesc}`,
        `\tpkgver = ${version}`,
        "\tpkgrel = 1",
        `\turl = ${HOMEPAGE}`,
        "\tarch = x86_64",
        "\tarch = aarch64",
        "\tlicense = GPL-3.0-only",
        ...common.depends.map((item) => `\tdepends = ${item}`),
        ...common.optdepends.map((item) => `\toptdepends = ${item}`),
        "\tprovides = coreverse-launcher",
        "\tconflicts = coreverse-launcher",
        "\toptions = !strip",
        "\toptions = !debug",
        ...debs.map((deb) => `\tnoextract = ${deb.name}`),
        ...debs.flatMap((deb) => [
            `\tsource_${deb.archLinux} = ${url(tag, deb.name)}`,
            `\tsha256sums_${deb.archLinux} = ${deb.sha256}`,
        ]),
        "",
        `pkgname = ${AUR_PACKAGE}`,
        "",
    ].join("\n");

    return { PKGBUILD: pkgbuild, ".SRCINFO": srcinfo };
}
