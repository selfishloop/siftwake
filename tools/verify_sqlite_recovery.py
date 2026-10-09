"""Acknowledged commit + real process kill and broad substring comparison."""
import json
import subprocess
import sys
import time
from pathlib import Path
from measure_sqlite import connect, fold, payload, timed_query

path=Path(sys.argv[2] if sys.argv[1]=='--worker' else sys.argv[1])/'sqlite.db'

if sys.argv[1]=='--worker':
    db=connect(path)
    doc=dict(key=dict(scope='shared',collection='contents',id='recovery-sentinel'),
             title='Durable recovery sentinel',text='process kill acknowledged',fields={},links=[])
    with db:
        db.execute('INSERT INTO docs(scope,collection,id,source,day,payload,folded) VALUES(?,?,?,?,?,?,?)',
                   ('shared','contents','recovery-sentinel','',0,payload(doc,2002),fold(doc['title']+'\n'+doc['text'])))
    print(json.dumps(dict(acknowledged=True,version=2002)),flush=True)
    sys.stdin.readline()  # Parent kills the process after reading the acknowledgment.
    sys.exit(0)

count=int(sys.argv[2])
started=time.perf_counter()
child=subprocess.Popen([sys.executable,'-X','utf8',__file__,'--worker',str(path.parent)],
                       stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf-8')
ack=json.loads(child.stdout.readline())
assert ack['acknowledged']
child.kill()
child.wait(timeout=10)
child.stdin.close()
child.stdout.close()
db=connect(path)
assert db.execute('SELECT count(*) FROM docs').fetchone()[0]==count-49
stored=json.loads(db.execute("SELECT payload FROM docs WHERE id='recovery-sentinel'").fetchone()[0])
assert stored['version']==2002 and stored['document']['text']=='process kill acknowledged'
for n in range(100):
    row=db.execute("SELECT payload FROM docs WHERE scope='shared' AND collection='contents' AND id=?",(f'{n*(count//100):010}',)).fetchone()
    if n%2==0:
        assert row is None
    else:
        assert json.loads(row[0])['document']['text']=='changed sentinel'
broad,_=timed_query(db,'text',count-100,3,text='teknoloji')
db.close()
print(json.dumps(dict(process_kill_recovery='passed',documents=count-49,verified_updates=50,
                      verified_deletes=50,broad_text_query='teknoloji',broad_text_total=count-100,
                      broad_text=broad,elapsed_s=time.perf_counter()-started)))
