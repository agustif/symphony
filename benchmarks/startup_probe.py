#!/usr/bin/env python3
import argparse
import os
import signal
import subprocess
import sys
import time
import tempfile
import urllib.error
import urllib.request


def wait_for_http(url: str, timeout_seconds: float, interval_seconds: float) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1) as response:
                if 200 <= response.status < 500:
                    return
        except urllib.error.URLError:
            pass
        except TimeoutError:
            pass
        time.sleep(interval_seconds)

    raise TimeoutError(f"timed out waiting for {url}")


def terminate_process_group(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return

    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return

    try:
        process.wait(timeout=5)
        return
    except subprocess.TimeoutExpired:
        pass

    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        return

    process.wait(timeout=5)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", required=True)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--interval-seconds", type=float, default=0.1)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()

    command = args.command
    if command and command[0] == "--":
        command = command[1:]

    if not command:
        wait_for_http(args.url, args.timeout_seconds, args.interval_seconds)
        return 0

    with tempfile.NamedTemporaryFile(mode="w+", delete=False) as log_file:
        log_path = log_file.name

    log_handle = open(log_path, "w", encoding="utf-8")
    process = subprocess.Popen(
        command,
        stdout=log_handle,
        stderr=subprocess.STDOUT,
        preexec_fn=os.setsid,
        text=True,
    )

    try:
        wait_for_http(args.url, args.timeout_seconds, args.interval_seconds)
    except Exception as error:
        terminate_process_group(process)
        with open(log_path, "r", encoding="utf-8", errors="replace") as handle:
            sys.stderr.write(handle.read())
        log_handle.close()
        os.unlink(log_path)
        raise SystemExit(str(error))

    terminate_process_group(process)
    log_handle.close()
    os.unlink(log_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
