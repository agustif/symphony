#!/usr/bin/env python3
import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


DEFAULT_STATE_CYCLE = ("Todo", "In Progress", "Done", "Closed")


def default_issue(index: int, state_name: str) -> dict[str, Any]:
    issue_number = index + 1
    issue_id = f"issue-{issue_number}"
    identifier = f"BENCH-{issue_number:04d}"
    return {
        "id": issue_id,
        "identifier": identifier,
        "title": f"Benchmark issue {identifier}",
        "description": "Static issue served by the local benchmark Linear stub.",
        "priority": 0,
        "state": {"name": state_name},
        "branchName": None,
        "url": f"https://linear.local/{identifier}",
        "assignee": None,
        "labels": {"nodes": []},
        "inverseRelations": {"nodes": []},
        "createdAt": "2026-03-07T00:00:00Z",
        "updatedAt": "2026-03-07T00:00:00Z",
    }


def default_issues(count: int, state_cycle: tuple[str, ...]) -> list[dict[str, Any]]:
    return [default_issue(index, state_cycle[index % len(state_cycle)]) for index in range(count)]


def load_issues(path: str | None, issue_count: int, state_cycle: tuple[str, ...]) -> list[dict[str, Any]]:
    if path is None:
        return default_issues(issue_count, state_cycle)
    with open(path, "r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, list):
        raise ValueError("issues fixture must be a JSON array")
    return payload


def page_info(nodes: list[dict[str, Any]], first: int, after: str | None) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    offset = int(after or "0")
    next_offset = offset + first
    page = nodes[offset:next_offset]
    has_next_page = next_offset < len(nodes)
    return page, {"hasNextPage": has_next_page, "endCursor": str(next_offset) if has_next_page else None}


class FakeLinearHandler(BaseHTTPRequestHandler):
    issues: list[dict[str, Any]] = []
    project_slug: str = "bench-project"

    def log_message(self, format: str, *args: Any) -> None:
        return

    def _send_json(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path == "/health":
            self._send_json(200, {"ok": True})
            return
        self._send_json(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path != "/graphql":
            self._send_json(404, {"error": "not found"})
            return

        content_length = int(self.headers.get("Content-Length", "0"))
        payload = json.loads(self.rfile.read(content_length) or b"{}")
        query = payload.get("query", "")
        variables = payload.get("variables") or {}

        if "viewer" in query:
            self._send_json(200, {"data": {"viewer": {"id": "benchmark-viewer"}}})
            return

        issues = list(self.issues)
        project_slug = variables.get("projectSlug")
        if project_slug is not None and project_slug != self.project_slug:
            issues = []

        state_names = variables.get("states") or variables.get("stateNames")
        if state_names:
            wanted = {str(name).strip().lower() for name in state_names}
            issues = [
                issue
                for issue in issues
                if issue.get("state", {}).get("name", "").strip().lower() in wanted
            ]

        ids = variables.get("ids")
        if ids:
            wanted_ids = {str(issue_id) for issue_id in ids}
            issues = [issue for issue in issues if str(issue.get("id")) in wanted_ids]

        first = int(variables.get("first") or len(issues) or 1)
        after = variables.get("after")
        page, page_info_payload = page_info(issues, first, after)
        self._send_json(
            200,
            {
                "data": {
                    "issues": {
                        "nodes": page,
                        "pageInfo": page_info_payload,
                    }
                }
            },
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=4010)
    parser.add_argument("--project-slug", default="bench-project")
    parser.add_argument("--issues-json")
    parser.add_argument("--issue-count", type=int, default=200)
    args = parser.parse_args()

    FakeLinearHandler.project_slug = args.project_slug
    FakeLinearHandler.issues = load_issues(args.issues_json, args.issue_count, DEFAULT_STATE_CYCLE)

    server = ThreadingHTTPServer((args.host, args.port), FakeLinearHandler)
    server.serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
