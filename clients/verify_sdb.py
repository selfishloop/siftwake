"""Verify an imported sample against the real, read-only source TSV."""
import csv
import sys
from siftwake_client import SiftwakeClient

executable, database, source, namespace = sys.argv[1:]
with open(source, encoding="utf-8-sig", newline="") as stream:
    first = next(row for row in csv.DictReader(stream, delimiter="\t", quoting=csv.QUOTE_NONE) if row["id"] not in {"0", "5"})

with SiftwakeClient(executable, database) as db:
    result = db.request("search", query={"scope": "shared", "collection": "files", "exact": {"volume": namespace}, "limit": 1})
    assert result["total"] == 1000
    found = db.request("search", query={"scope": "shared", "collection": "files", "text": first["name"], "limit": 1000})
    source_match = [h for h in found["hits"] if h["document"]["fields"]["file_id"] == first["id"] and h["document"]["title"] == first["name"]]
    assert source_match
    assert source_match[0]["document"]["fields"]["parent_id"] == first["parent_id"]
    assert source_match[0]["document"]["fields"]["flags"] == int(first["flags"])
    assert db.request("search", query={"scope": "user:unrelated"})["total"] == 0
    db.request("compact")

with SiftwakeClient(executable, database) as db:
    assert db.request("get", key=source_match[0]["document"]["key"])["document"]["title"] == first["name"]
    print("SDB: 1000 real metadata records; name search, fields, scope, compact and reopen: OK")
