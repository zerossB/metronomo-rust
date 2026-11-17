#!/usr/bin/env bash
# Run only inside a disposable, clean Ubuntu container/VM as root.
set -euo pipefail
trap 'printf "Package installation check failed at line %s\n" "$LINENO" >&2' ERR
if [[ "$(id -u)" != 0 || ! -f /run/.containerenv && ! -f /.dockerenv ]]; then
    echo 'This script installs/removes packages; run it inside a disposable container.' >&2
    exit 1
fi
package="$(realpath "${1:?Pass the candidate .deb}")"
evidence="${2:?Pass an evidence directory}"
if command -v rustc >/dev/null || command -v cargo >/dev/null; then
    echo 'The clean-install test requires an environment without Rust.' >&2
    exit 1
fi
mkdir -p "$evidence"
# Ubuntu's minimal OCI image filters most /usr/share/doc files. A normal desktop
# does not; include this package's documentation explicitly for the content check.
printf '%s\n' 'path-include=/usr/share/doc/metronomo' 'path-include=/usr/share/doc/metronomo/*' \
    > /etc/dpkg/dpkg.cfg.d/zz-metronomo-validation
apt-get update
apt-get install -y --no-install-recommends "$package"
test -x /usr/bin/metronomo
test -f /usr/share/applications/com.haynestec.metronomo.desktop
test -f /usr/share/icons/hicolor/scalable/apps/metronomo.svg
test -s /usr/share/doc/metronomo/third-party-notices.txt
test -f /usr/share/doc/metronomo/build-provenance.json
test -f /usr/share/doc/metronomo/docs/release.md
ldd /usr/bin/metronomo > "$evidence/linked-libraries.txt"
if rg_missing="$(sed -n '/not found/p' "$evidence/linked-libraries.txt")"; [[ -n "$rg_missing" ]]; then
    echo "$rg_missing" >&2
    exit 1
fi
dpkg-query -W > "$evidence/installed-packages.txt"
export XDG_CONFIG_HOME
XDG_CONFIG_HOME="$(mktemp -d)"
trap 'rm -rf "$XDG_CONFIG_HOME"' EXIT
mkdir -p "$XDG_CONFIG_HOME/metronomo"
printf '%s\n' 'package lifecycle retention sentinel; not app startup evidence' > "$XDG_CONFIG_HOME/metronomo/settings.json"
cp "$XDG_CONFIG_HOME/metronomo/settings.json" "$XDG_CONFIG_HOME/expected"
apt-get install -y --reinstall --no-install-recommends "$package"
cmp "$XDG_CONFIG_HOME/expected" "$XDG_CONFIG_HOME/metronomo/settings.json"
apt-get remove -y metronomo
test ! -e /usr/bin/metronomo
test ! -e /usr/share/applications/com.haynestec.metronomo.desktop
cmp "$XDG_CONFIG_HOME/expected" "$XDG_CONFIG_HOME/metronomo/settings.json"
sha256sum "$package" > "$evidence/package.sha256"
cp /etc/os-release "$evidence/os-release"
cat > "$evidence/result.json" <<'EOF'
{
  "passed": true,
  "measurement": "clean disposable container installation, linked library resolution, reinstall and removal",
  "rust_required": false,
  "preferences_marker_preserved": true,
  "desktop_launch": "not measured",
  "audio_playback": "not measured"
}
EOF
