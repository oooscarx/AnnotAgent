"""Apply reversible navigation-only archival after an SQLite online backup.
Usage: python3 scripts/organize_presentation_sessions.py BACKUP_DIRECTORY
The backup directory must contain tasks-before.json and history-before.db.
"""
from pathlib import Path
import datetime, json, sqlite3, sys
backup = Path(sys.argv[1]).resolve()
db_path = backup.parent.parent / 'history.db'
keep = '357155d7-3073-4dea-b07b-90adfa10ed4e'
expected = json.loads((backup / 'tasks-before.json').read_text())
assert keep in [row[0] for row in expected]
c = sqlite3.connect(db_path, timeout=10)
c.execute('PRAGMA foreign_keys=ON')
c.execute('BEGIN IMMEDIATE')
try:
    current = c.execute("SELECT t.id,t.conversation_id,t.source_message_id,json_extract(m.input_json,'$.text') FROM conversation_tasks t JOIN conversation_messages m ON m.conversation_id=t.conversation_id AND m.message_id=t.source_message_id").fetchall()
    assert sorted(map(tuple,expected)) == sorted(current), 'Task list changed after backup'
    for table, statuses in [('conversation_model_calls', ('reserved',)), ('conversation_builder_operations', ('reserved',)), ('sample_operations', ('queued','running','cancelling'))]:
        marks=','.join('?' for _ in statuses)
        assert c.execute(f'SELECT count(*) FROM {table} WHERE status IN ({marks})', statuses).fetchone()[0] == 0, 'An operation is active'
    c.execute((Path(__file__).resolve().parent.parent / 'migrations/conversation_task_display.sql').read_text())
    stamp=datetime.datetime.now(datetime.timezone.utc).isoformat()
    previous=[]
    for task,*_ in current:
        previous.append({'task_id':task,'value':c.execute('SELECT title,archived_at,updated_at FROM conversation_task_display WHERE task_id=?',(task,)).fetchone()})
        c.execute('INSERT INTO conversation_task_display(task_id,title,archived_at,updated_at) VALUES(?,?,?,?) ON CONFLICT(task_id) DO UPDATE SET title=excluded.title,archived_at=excluded.archived_at,updated_at=excluded.updated_at',
                  (task,'B-Human 足球标注' if task==keep else None,None if task==keep else stamp,stamp))
    # A restore script only removes these exact navigation overrides; no history rollback.
    restore=['BEGIN IMMEDIATE;']
    for row in previous:
        task=row['task_id']
        restore.append(f"DELETE FROM conversation_task_display WHERE task_id='{task}' AND updated_at='{stamp}';")
        if row['value']:
            quoted=["NULL" if x is None else "'"+x.replace("'","''")+"'" for x in row['value']]
            restore.append(f"INSERT OR IGNORE INTO conversation_task_display VALUES('{task}',{','.join(quoted)});")
    restore.append('COMMIT;')
    (backup/'restore-navigation.sql').write_text('\n'.join(restore))
    (backup/'navigation-change.json').write_text(json.dumps({'keep':keep,'archived':[r[0] for r in current if r[0]!=keep],'previous':previous,'updated_at':stamp},ensure_ascii=False,indent=2))
    c.commit()
    print(f'Archived {len(current)-1} sessions; retained {keep}')
except:
    c.rollback()
    raise
