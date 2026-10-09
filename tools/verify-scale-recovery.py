"""Kill a real stdio owner after an acknowledged WAL write on the scale corpus."""
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "clients"))
from fdb_client import FdbClient

binary, directory, count = sys.argv[1], sys.argv[2], int(sys.argv[3])
key = {"scope": "shared", "collection": "contents", "id": "recovery-sentinel"}
start = time.perf_counter()
client = FdbClient(binary, directory)
before = client.request("stats")
assert before["documents"] == count - 50
assert client.request("get", key=key) is None
ack = client.request("commit", mutations=[{
    "op": "put", "guard": {"kind": "missing"},
    "document": {"key": key, "title": "Durable recovery sentinel", "text": "process kill acknowledged"},
}])
client._process.kill()
client._process.wait(timeout=10)
client.close()
with FdbClient(binary, directory) as client:
    after = client.request("stats")
    stored = client.request("get", key=key)
    assert after["sequence"] == ack["sequence"] == before["sequence"] + 1
    assert after["documents"] == count - 49
    assert stored["version"] == ack["sequence"]
    assert stored["document"]["text"] == "process kill acknowledged"
    for n in range(100):
        k = {"scope": "shared", "collection": "contents", "id": f"{n*(count//100):010}"}
        result = client.request("get", key=k)
        if n % 2 == 0:
            assert result is None
        else:
            assert result["document"]["text"] == "changed sentinel"
    broad_text_ms = []
    for _ in range(3):
        query_start = time.perf_counter()
        page = client.request("search", query={"scope": "shared", "collection": "contents", "text": "teknoloji", "limit": 50})
        broad_text_ms.append((time.perf_counter() - query_start) * 1000)
        assert page["total"] == count - 100
print(json.dumps({"process_kill_recovery": "passed", "elapsed_s": time.perf_counter()-start,
                  "sequence": ack["sequence"], "documents": after["documents"],
                  "verified_prior_updates": 50, "verified_prior_deletes": 50,
                  "broad_text_query": "teknoloji", "broad_text_total": count - 100,
                  "broad_text_ms": broad_text_ms}))
