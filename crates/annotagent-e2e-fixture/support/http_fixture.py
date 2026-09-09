#!/usr/bin/env python3
"""Opt-in real HTTP integration server + existing external-model fixture.

No third-party Python packages. Never reads user credential files. Ctrl-C only
stops owned children; the marked temporary workspace and evidence are retained.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request
import urllib.parse

ROOT = Path(__file__).resolve().parents[3]
MARKER = "AnnotAgent HTTP integration fixture\n"


def free_port(value):
    if value == 8787:
        raise ValueError("8787 is forbidden")
    with socket.socket() as sock:
        # Match the server bind: allow an owned prior socket in TIME_WAIT,
        # but never share an active listener (SO_REUSEPORT is not enabled).
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.bind(("127.0.0.1", value))
        # Probe a listener, not just a bound socket. The child still performs
        # the authoritative bind after this short-lived probe is closed.
        sock.listen(1)
        port = sock.getsockname()[1]
    if port == 8787:
        return free_port(0)
    return port


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--enable-fixture", action="store_true", required=True)
    parser.add_argument("--workspace", type=Path, help="Reuse only a previously marked TEST-agent-ui-* temp directory")
    parser.add_argument("--port", type=int, default=0)
    parser.add_argument("--provider-port", type=int, default=0)
    parser.add_argument("--web-dist", type=Path, help="Optional existing build to serve on the same origin; never built or edited")
    parser.add_argument("--smoke", action="store_true", help="Seed, verify, then stop owned servers")
    args = parser.parse_args()
    workspace = (args.workspace or Path(tempfile.mkdtemp(prefix="TEST-agent-ui-"))).resolve()
    if not workspace.is_relative_to(Path(tempfile.gettempdir()).resolve()) or not workspace.name.startswith("TEST-agent-ui-"):
        parser.error("workspace must be a marked TEST-agent-ui-* directory under system temp")
    marker = workspace / "FIXTURE_ONLY"
    if args.workspace:
        if not marker.is_file() or marker.read_text() != MARKER:
            parser.error("refusing an unmarked existing workspace")
    else:
        marker.write_text(MARKER)
    previous = json.loads((workspace / "manifest.json").read_text()) if args.workspace else None
    if previous and "restart_snapshot" not in previous:
        parser.error("previous seed did not complete; start a fresh fixture without --workspace")
    api_port = free_port(args.port or (urllib.parse.urlparse(previous["base_url"]).port if previous else 0))
    previous_provider_port = urllib.parse.urlparse(previous["provider_url"]).port if previous else 0
    if previous and args.provider_port not in [0, previous_provider_port]:
        parser.error("restart must retain provider port so frozen approvals remain valid")
    provider_port = free_port(args.provider_port or previous_provider_port)
    if api_port == provider_port:
        parser.error("API and provider ports must differ")
    # Allowlist build essentials; no inherited provider tokens or credential env.
    env = {key: os.environ[key] for key in ("PATH", "HOME", "USER", "TMPDIR", "RUSTUP_HOME", "CARGO_HOME", "SDKROOT", "DEVELOPER_DIR") if key in os.environ}
    env["CARGO_TARGET_DIR"] = str(ROOT / "target")
    build = subprocess.run(["cargo", "test", "-p", "annotagent-server", "--lib", "--offline", "--no-run", "--message-format=json"], cwd=ROOT, env=env, stdout=subprocess.PIPE, text=True, check=True)
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
    executable = next(item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact" and item.get("profile", {}).get("test") and item.get("target", {}).get("name") == "annotagent_server" and item.get("executable"))
    subprocess.run(["cargo", "build", "--offline", "-p", "annotagent-e2e-fixture"], cwd=ROOT, env=env, check=True)
    manifest = {**(previous or {}), "fixture": "external-model-only", "workspace": str(workspace), "base_url": f"http://127.0.0.1:{api_port}", "provider_url": f"http://127.0.0.1:{provider_port}/openai/v1", "backend_sha": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()}
    (workspace / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    children = []
    logs = []
    try:
        for name, command, extra in [
            ("provider", [str(ROOT / "target/debug/annotagent-e2e-fixture")], {"ANNOTAGENT_E2E_WORKER_PORT": str(provider_port)}),
            ("server", [executable, "--exact", "tests::integration_http_fixture", "--ignored", "--nocapture"], {"ANNOTAGENT_TEST_HTTP_FIXTURE": json.dumps({"enabled": True, "workspace": str(workspace), "port": api_port, "provider_port": provider_port, "web_dist": str(args.web_dist.resolve()) if args.web_dist else None})}),
        ]:
            log = (workspace / f"{name}.log").open("a")
            logs.append(log)
            children.append(subprocess.Popen(command, cwd=workspace, env={**env, **extra}, stdout=log, stderr=subprocess.STDOUT))
        for url in [f"http://127.0.0.1:{provider_port}/health", manifest["base_url"] + "/api/health"]:
            for _ in range(150):
                if any(child.poll() is not None for child in children):
                    raise RuntimeError(f"owned child exited; inspect {workspace}/*.log")
                try:
                    with urllib.request.urlopen(url, timeout=1) as response:
                        if "/api/" in url:
                            assert response.headers.get("x-annotagent-fixture") == "external-model-only"
                    break
                except OSError:
                    time.sleep(0.2)
            else:
                raise RuntimeError(f"startup timeout: {url}")
        from http_smoke import Client, seed_and_verify, restart_snapshot, verify_sse
        if previous:
            client = Client(manifest["base_url"])
            snapshot = restart_snapshot(client, manifest)
            unchanged = snapshot == previous["restart_snapshot"]
            if args.smoke:
                assert unchanged, "Persisted state differs from seed snapshot; use a fresh fixture for passive-restart checks"
            manifest["seed_snapshot_unchanged"] = unchanged
            manifest["restart_snapshot"] = snapshot
            verify_sse(client, manifest["run_id"])
            manifest["restart_verified"] = True
            (workspace / "RESTART_TRACE.json").write_text(json.dumps(client.trace, indent=2, ensure_ascii=False) + "\n")
        else:
            manifest.update(seed_and_verify(manifest, ROOT))
            manifest["restart_snapshot"] = restart_snapshot(Client(manifest["base_url"]), manifest)
        (workspace / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(json.dumps({key: value for key, value in manifest.items() if key not in ["restart_snapshot", "stop", "export"]}, indent=2), flush=True)
        print(f"TEST fixture ready. Manifest: {workspace / 'manifest.json'}; Ctrl-C stops only owned children, retains DB.", flush=True)
        if not args.smoke:
            while all(child.poll() is None for child in children):
                time.sleep(0.5)
            raise RuntimeError(f"owned child exited; inspect {workspace}/*.log")
    finally:
        for child in reversed(children):
            if child.poll() is None:
                child.send_signal(signal.SIGINT)
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        for log in logs:
            log.close()


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
