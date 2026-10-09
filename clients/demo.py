"""Run: python clients/demo.py target/release/fdb.exe data/python-demo"""
import sys
from fdb_client import FdbClient

with FdbClient(sys.argv[1], sys.argv[2]) as db:
    key = {"scope": "shared", "collection": "kap_reports", "id": "sample-python-report"}
    print(db.request("commit", mutations=[{
        "op": "put", "document": {
            "key": key, "title": "Örnek KAP raporu",
            "text": "Şirketin teknoloji yatırımları", "fields": {"fund_code": "ABC", "as_of": 20261007},
            "links": [{"scope": "shared", "collection": "funds", "id": "ABC"}],
        },
    }]))
    result = db.request("search", query={"scope": "shared", "text": "sirketin", "exact": {"fund_code": "ABC"}})
    assert result["total"] == 1
    print(result)
    db.request("compact")
    db.request("wait_compaction")

with FdbClient(sys.argv[1], sys.argv[2]) as db:
    assert db.request("get", key=key)["document"]["fields"]["fund_code"] == "ABC"
    print("Python roundtrip and durable reopen: OK")
