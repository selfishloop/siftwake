"""Local stdio client for Python workers. No database package or live credentials."""
from __future__ import annotations
import json
import subprocess
from pathlib import Path
from threading import Lock


class FdbClient:
    def __init__(self, executable: str | Path, database: str | Path):
        self._lock = Lock()
        self._process = subprocess.Popen(
            [str(executable), str(database), "--stdio"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, encoding="utf-8", bufsize=1,
        )

    def request(self, op: str, **arguments):
        with self._lock:
            if self._process.poll() is not None:
                raise RuntimeError("FDB process is closed")
            self._process.stdin.write(json.dumps({"op": op, **arguments}, ensure_ascii=False) + "\n")
            self._process.stdin.flush()
            line = self._process.stdout.readline()
            if not line:
                raise RuntimeError("FDB connection ended; outcome of an in-flight write is unknown")
            response = json.loads(line)
            if not response["ok"]:
                raise RuntimeError(response["error"])
            return response["result"]

    def close(self):
        with self._lock:
            if self._process.stdin and not self._process.stdin.closed:
                self._process.stdin.close()
            self._process.wait(timeout=10)
            self._process.stdout.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
