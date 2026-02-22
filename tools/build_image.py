#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import logging
import shutil
import subprocess
import tarfile
import textwrap
import zipfile
from pathlib import Path
from typing import Iterable

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python <3.11
    import tomli as tomllib  # type: ignore


LOGGER = logging.getLogger("build_image")
REPO_ROOT = Path(__file__).resolve().parent.parent
NODE_AGENT_ROOT = REPO_ROOT / "apps" / "node-agent"
DEFAULT_SERVICE_USER = "farmnode"


def main() -> None:
    logging.basicConfig(level=logging.INFO, format="%(message)s")

    parser = argparse.ArgumentParser(description="Package Raspberry Pi images for the Farm Node Agent.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    default_workspace = REPO_ROOT / "build" / "pi-node"

    imager = subparsers.add_parser(
        "pi-imager-profile",
        help="Generate a versioned Pi 5 node image kit (overlay + Pi Imager first-run script + checksums).",
    )
    imager.add_argument("--workspace", type=Path, default=default_workspace / "imager", help="Where to render imager assets.")
    imager.add_argument("--name", default="Farm Node Agent (64-bit)", help="Display name for the profile.")
    imager.add_argument("--description", default="Raspberry Pi OS with Farm Node Agent pre-installed.", help="Description for the profile.")
    imager.add_argument("--base-image-url", default="https://downloads.raspberrypi.com/raspios_lite_arm64/images/raspios_lite_arm64-2024-07-04/2024-07-04-raspios-bookworm-arm64-lite.img.xz", help="Base OS download URL referenced in the profile.")
    imager.add_argument("--force", action="store_true", help="Remove existing workspace before generating.")
    imager.add_argument("--no-zip", action="store_true", help="Do not create a bundled .zip kit (dist/ will still be populated).")

    args = parser.parse_args()

    if args.command == "pi-imager-profile":
        generate_pi_imager_profile(args)
    else:  # pragma: no cover - argparse guards this
        parser.error(f"Unknown command {args.command}")


def prepare_overlay(root: Path, version: str, git_commit: str, clean: bool = False) -> None:
    if clean and root.exists():
        shutil.rmtree(root)
    (root / "etc/systemd/system").mkdir(parents=True, exist_ok=True)
    (root / "etc/logrotate.d").mkdir(parents=True, exist_ok=True)
    (root / "usr/local/bin").mkdir(parents=True, exist_ok=True)
    (root / "opt").mkdir(parents=True, exist_ok=True)

    copy_node_agent_source(root / "opt" / "node-agent")
    write_node_agent_build_info(root / "opt" / "node-agent" / "app" / "build_info.py", flavor="prod")
    write_node_agent_version_file(root / "opt" / "node-agent" / "VERSION", version, git_commit)
    requirements_path = root / "opt" / "node-agent" / "requirements.txt"
    export_requirements(NODE_AGENT_ROOT, requirements_path)
    stage_offline_node_deps(
        vendor_dir=root / "opt" / "node-agent" / "vendor",
        debs_dir=root / "opt" / "node-agent" / "debs",
    )
    copy_systemd_units(root / "etc" / "systemd" / "system")
    copy_logrotate_configs(root / "etc" / "logrotate.d")
    copy_scripts(root / "usr" / "local" / "bin")
    copy_env_sample(root)


def copy_node_agent_source(dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    ignore = shutil.ignore_patterns("__pycache__", "*.pyc", "*.pyo", ".pytest_cache", "tests", "systemd", "scripts")
    shutil.copytree(NODE_AGENT_ROOT, dest, ignore=ignore)


def write_node_agent_build_info(path: Path, *, flavor: str) -> None:
    if flavor not in {"prod", "dev", "test"}:
        raise ValueError(f"Invalid node-agent build flavor: {flavor}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "from __future__ import annotations",
                "",
                "from typing import Literal",
                "",
                f'BUILD_FLAVOR: Literal["prod", "dev", "test"] = "{flavor}"',
                "",
            ]
        ),
        encoding="utf-8",
    )


def copy_systemd_units(dest: Path) -> None:
    systemd_src = NODE_AGENT_ROOT / "systemd"
    unit_patterns = ("node-agent*.service", "node-agent*.timer", "node-agent*.path", "renogy-bt.service")
    for pattern in unit_patterns:
        for src in systemd_src.glob(pattern):
            shutil.copy2(src, dest / src.name)


def copy_logrotate_configs(dest: Path) -> None:
    logrotate_src = NODE_AGENT_ROOT / "systemd" / "logrotate"
    if not logrotate_src.exists():
        return
    for src in logrotate_src.glob("*"):
        if src.is_file():
            shutil.copy2(src, dest / src.name)


def copy_scripts(dest: Path) -> None:
    scripts_src = NODE_AGENT_ROOT / "scripts"
    scripts_map = {
        "node-agent-python.sh": "node-agent-python",
        "node-agent-logrotate.sh": "node-agent-logrotate",
        "node-agent-optional-services.py": "node-agent-optional-services",
        "verify_backups.py": "node-agent-verify-backups",
    }
    for source_name, dest_name in scripts_map.items():
        src = scripts_src / source_name
        if not src.exists():
            continue
        target = dest / dest_name
        shutil.copy2(src, target)
        target.chmod(0o755)


def copy_env_sample(root: Path) -> None:
    env_sample = NODE_AGENT_ROOT / "systemd" / "node-agent.env.sample"
    if env_sample.exists():
        shutil.copy2(env_sample, root / "etc" / "node-agent.env.example")


def write_node_agent_version_file(path: Path, version: str, git_commit: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    content = "\n".join(
        [
            version,
            f"git_commit={git_commit}",
            "",
        ]
    )
    path.write_text(content, encoding="utf-8")


def export_requirements(project_root: Path, dest: Path) -> None:
    try:
        subprocess.run(
            ["poetry", "export", "-f", "requirements.txt", "--without-hashes", "-o", str(dest)],
            check=True,
            cwd=project_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        LOGGER.info("Generated requirements.txt via poetry export")
    except (FileNotFoundError, subprocess.CalledProcessError):
        LOGGER.warning("poetry export failed; falling back to pyproject dependency parsing")
        project = tomllib.loads((project_root / "pyproject.toml").read_text())
        deps: Iterable[str] = project.get("project", {}).get("dependencies", [])
        normalized = [normalize_requirement(dep) for dep in deps]
        dest.write_text("\n".join(normalized) + "\n", encoding="utf-8")


def stage_offline_node_deps(*, vendor_dir: Path, debs_dir: Path) -> None:
    """Build/stage everything needed to install node-agent on a Pi with no WAN.

    - Python deps are vendored under /opt/node-agent/vendor (Pi-targeted wheels).
    - pigpio debs are staged under /opt/node-agent/debs for dpkg install during first boot.
    """

    vendor_dir.parent.mkdir(parents=True, exist_ok=True)
    debs_dir.parent.mkdir(parents=True, exist_ok=True)

    cache_root = REPO_ROOT / "build" / "cache"
    cache_root.mkdir(parents=True, exist_ok=True)
    helper = REPO_ROOT / "tools" / "build_node_offline_deps.py"
    subprocess.run(
        [
            "python3",
            str(helper),
            "--vendor-dir",
            str(vendor_dir),
            "--debs-dir",
            str(debs_dir),
            "--cache-root",
            str(cache_root),
        ],
        check=True,
    )


def normalize_requirement(requirement: str) -> str:
    requirement = requirement.strip()
    if "(" not in requirement:
        return requirement
    name, rest = requirement.split("(", 1)
    if ")" not in rest:
        return requirement
    spec, tail = rest.split(")", 1)
    spec = spec.replace(" ", "")
    normalized = f"{name.strip()}{spec}"
    tail = tail.strip()
    if tail:
        normalized = f"{normalized} {tail}"
    return normalized


def create_overlay_tar(source: Path, dest: Path) -> None:
    if dest.exists():
        dest.unlink()
    with tarfile.open(dest, "w:gz") as tar:
        tar.add(source, arcname=".")


def generate_pi_imager_profile(args: argparse.Namespace) -> None:
    workspace: Path = args.workspace.expanduser().resolve()
    LOGGER.info("Preparing Raspberry Pi Imager assets at %s", workspace)
    if args.force and workspace.exists():
        shutil.rmtree(workspace)

    semver = read_node_agent_semver()
    git_commit = read_git_commit()
    version = build_version_string(semver, git_commit)

    overlay_root = workspace / "overlay"
    prepare_overlay(overlay_root, version=version, git_commit=git_commit, clean=True)
    dist_dir = workspace / "dist"
    dist_dir.mkdir(parents=True, exist_ok=True)

    overlay_tar = dist_dir / "node-agent-overlay.tar.gz"
    create_overlay_tar(overlay_root, overlay_tar)

    firstrun_script = dist_dir / "node-agent-firstrun.sh"
    write_firstrun_script(
        firstrun_script,
        overlay_tar=overlay_tar,
        version=version,
        git_commit=git_commit,
        service_user=DEFAULT_SERVICE_USER,
    )

    profile_path = dist_dir / "node-agent-imager.json"
    profile = {
        "name": args.name,
        "description": args.description,
        "base_image_url": args.base_image_url,
        "artifacts": {
            "overlay_archive": overlay_tar.name,
            "firstrun_script": firstrun_script.name,
        },
        "instructions": [
            "Open Raspberry Pi Imager and select Raspberry Pi OS Lite (64-bit).",
            "Open Imager advanced options (gear icon) and enable OS customization as needed (username/password, Wi-Fi, locale).",
            f"Enable 'Run custom script on first boot' and choose {firstrun_script.name}.",
            "Flash, boot the Pi 5, then adopt the node from the dashboard (scan/adopt).",
        ],
    }
    profile_path.write_text(json.dumps(profile, indent=2), encoding="utf-8")
    LOGGER.info("Profile JSON written to %s", profile_path)

    version_path = dist_dir / "VERSION"
    write_node_agent_version_file(version_path, version, git_commit)

    zip_path = None
    if not args.no_zip:
        slug = version.replace("+", "_")
        zip_path = dist_dir / f"pi5-node-image-kit-{slug}.zip"
        write_kit_zip(
            zip_path,
            files=[
                overlay_tar,
                firstrun_script,
                profile_path,
                version_path,
            ],
        )
        LOGGER.info("Kit zip written to %s", zip_path)

    sha_path = dist_dir / "SHA256SUMS"
    write_sha256sums(sha_path, dist_dir, extra_files=[])
    LOGGER.info("Checksums written to %s", sha_path)


def build_version_string(semver: str, git_commit: str) -> str:
    commit = git_commit.strip()
    if commit:
        return f"{semver}+{commit[:12]}"
    return semver


def read_git_commit() -> str:
    try:
        completed = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            check=True,
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        return completed.stdout.strip()
    except Exception:
        return ""


def read_node_agent_semver() -> str:
    project = tomllib.loads((NODE_AGENT_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    version = project.get("project", {}).get("version")
    if not version:
        raise ValueError("Unable to determine node-agent version from apps/node-agent/pyproject.toml")
    return str(version).strip()


def write_firstrun_script(
    path: Path,
    *,
    overlay_tar: Path,
    version: str,
    git_commit: str,
    service_user: str,
) -> None:
    overlay_bytes = overlay_tar.read_bytes()
    header = textwrap.dedent(
        f"""\
        #!/bin/bash
        set -euo pipefail

        KIT_VERSION="{version}"
        KIT_GIT_COMMIT="{git_commit}"
        SERVICE_USER="{service_user}"

        log() {{
          echo "[farm-node-firstboot] $1"
        }}

        detect_boot_dir() {{
          if [ -d /boot/firmware ] && [ -f /boot/firmware/cmdline.txt ]; then
            echo "/boot/firmware"
            return
          fi
          echo "/boot"
        }}

        enable_spi() {{
          local boot_dir="$1"
          local cfg="${{boot_dir}}/config.txt"
          if [ ! -f "${{cfg}}" ]; then
            return
          fi
          if grep -Eq '^\\s*dtparam=spi=on\\s*$' "${{cfg}}"; then
            return
          fi
          log "Enabling SPI (dtparam=spi=on) in ${{cfg}}"
          if grep -Eq '^\\s*#\\s*dtparam=spi=on\\s*$' "${{cfg}}"; then
            sed -i -e 's/^\\s*#\\s*dtparam=spi=on\\s*$/dtparam=spi=on/' "${{cfg}}" || true
            return
          fi
          printf '\\n# FarmDashboard: enable SPI for ADS1263\\n' >> "${{cfg}}" || true
          printf 'dtparam=spi=on\\n' >> "${{cfg}}" || true
        }}

        cleanup_cmdline() {{
          local boot_dir="$1"
          local cmdline="${{boot_dir}}/cmdline.txt"
          if [ ! -f "${{cmdline}}" ]; then
            return
          fi
          sed -i \\
            -e 's| systemd.run=/boot/firstrun.sh||g' \\
            -e 's| systemd.run_success_action=reboot||g' \\
            -e 's| systemd.unit=kernel-command-line.target||g' \\
            "${{cmdline}}" || true
        }}

        extract_embedded_overlay() {{
          local out="$1"
          local marker="__FARM_NODE_OVERLAY_BELOW__"
          local start
          start="$(awk "/${{marker}}/{{print NR + 1; exit 0;}}" "$0")"
          if [ -z "${{start}}" ]; then
            log "ERROR: embedded overlay marker not found"
            exit 1
          fi
          tail -n +"${{start}}" "$0" > "${{out}}"
        }}

        ensure_service_user() {{
          if id -u "${{SERVICE_USER}}" >/dev/null 2>&1; then
            return
          fi
          log "Creating service user ${{SERVICE_USER}}"
          adduser --system --group --no-create-home "${{SERVICE_USER}}"
          for group in bluetooth dialout gpio i2c spi; do
            if getent group "${{group}}" >/dev/null 2>&1; then
              usermod -aG "${{group}}" "${{SERVICE_USER}}" || true
            fi
          done
        }}

        install_node_agent() {{
          local tmp_archive
          tmp_archive="$(mktemp /tmp/node-agent-overlay.XXXXXX.tar.gz)"
          log "Extracting node-agent overlay"
          extract_embedded_overlay "${{tmp_archive}}"
          tar -xzf "${{tmp_archive}}" -C /
          rm -f "${{tmp_archive}}"

          mkdir -p /opt/node-agent/storage
          chown -R "${{SERVICE_USER}}:${{SERVICE_USER}}" /opt/node-agent
        }}

        install_offline_debs() {{
          if [ -d /opt/node-agent/debs ] && ls /opt/node-agent/debs/*.deb >/dev/null 2>&1; then
            log "Installing offline deb packages"
            dpkg -i /opt/node-agent/debs/*.deb
          fi
        }}

        consume_optional_seed_files() {{
          local boot_dir="$1"
          local node_config="${{boot_dir}}/node_config.json"
          local node_firstboot="${{boot_dir}}/node-agent-firstboot.json"
          local node_env="${{boot_dir}}/node-agent.env"

          if [ -f "${{node_config}}" ]; then
            log "Applying node_config.json from boot volume"
            cp "${{node_config}}" /opt/node-agent/storage/node_config.json
            rm -f "${{node_config}}"
          fi
          if [ -f "${{node_firstboot}}" ]; then
            log "Applying node-agent-firstboot.json from boot volume"
            cp "${{node_firstboot}}" /opt/node-agent/storage/node-agent-firstboot.json
            rm -f "${{node_firstboot}}"
          fi
          if [ -f "${{node_env}}" ]; then
            log "Applying node-agent.env from boot volume"
            cp "${{node_env}}" /etc/node-agent.env
            rm -f "${{node_env}}"
          fi

          chown -R "${{SERVICE_USER}}:${{SERVICE_USER}}" /opt/node-agent/storage || true
        }}

        start_services() {{
          log "Enabling node-agent services"
          systemctl daemon-reload || true
          if systemctl list-unit-files | awk '{{print $1}}' | grep -qx 'pigpiod.service'; then
            systemctl enable --now pigpiod.service || true
          fi
          systemctl enable node-forwarder.service node-agent.service node-agent-logrotate.timer node-agent-backup-verify.timer node-agent-optional-services.path || true
          systemctl restart node-forwarder.service || true
          systemctl restart node-agent.service || true
          systemctl start node-agent-optional-services.service || true
        }}

        main() {{
          local boot_dir
          boot_dir="$(detect_boot_dir)"
          log "Using boot dir ${{boot_dir}}"

          enable_spi "${{boot_dir}}"

          ensure_service_user
          install_node_agent

          printf '%s\\n' "${{KIT_VERSION}}" "git_commit=${{KIT_GIT_COMMIT}}" > /opt/node-agent/VERSION
          chown "${{SERVICE_USER}}:${{SERVICE_USER}}" /opt/node-agent/VERSION || true

          install_offline_debs
          consume_optional_seed_files "${{boot_dir}}"
          start_services

          cleanup_cmdline "${{boot_dir}}"
          touch "${{boot_dir}}/farm-node-firstboot.done" || true
          rm -f /boot/firstrun.sh /boot/firmware/firstrun.sh || true
          log "First boot complete"
        }}

        main
        exit 0

        __FARM_NODE_OVERLAY_BELOW__
        """
    )
    with path.open("wb") as handle:
        handle.write(header.encode("utf-8"))
        handle.write(overlay_bytes)
    path.chmod(0o755)


def write_kit_zip(path: Path, *, files: list[Path]) -> None:
    if path.exists():
        path.unlink()
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zipf:
        for file_path in files:
            if file_path is None:
                continue
            zipf.write(file_path, arcname=file_path.name)


def write_sha256sums(path: Path, dist_dir: Path, *, extra_files: list[Path]) -> None:
    entries: list[tuple[str, str]] = []
    for file_path in sorted(dist_dir.iterdir(), key=lambda p: p.name):
        if not file_path.is_file():
            continue
        if file_path.name == path.name:
            continue
        entries.append((sha256_file(file_path), file_path.name))
    for extra in extra_files:
        if extra and extra.exists():
            entries.append((sha256_file(extra), extra.name))
    path.write_text("\n".join(f"{digest}  {name}" for digest, name in entries) + "\n", encoding="utf-8")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    main()
