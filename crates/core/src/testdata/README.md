# Test data

`comeback-library.db` is a SQLite database written by SQLite itself (Python's `sqlite3`, page size
1024), shaped like the library the Comebacks page downloads (`crate::comeback`): table
`main (id integer PRIMARY KEY NOT NULL, text varchar (255), level varchar (255) DEFAULT ('max'))`.
Its 140 rows are harmless words: `line 1` to `line 120` (odd `min`, even `max`), a line with
newlines and a tab, `dup` twice, a blank line, 1500 `长` (spills onto overflow pages), 201 `x`, a
row of another level, and `tail 1` to `tail 13`. It holds nothing of the real library.

To write it again:

```python
import sqlite3
con = sqlite3.connect("comeback-library.db")
con.execute("pragma page_size = 1024")
con.execute("CREATE TABLE main (id integer PRIMARY KEY NOT NULL, text varchar (255), level varchar (255) DEFAULT ('max'))")
rows = [(f"line {i}", "min" if i % 2 else "max") for i in range(1, 121)]
rows += [("\n两行\r\n文字\t", "min"), ("dup", "min"), ("dup", "max"), ("   ", "min"),
         ("长" * 1500, "max"), ("x" * 201, "min"), ("other", "neither")]
rows += [(f"tail {i}", "max") for i in range(1, 14)]
con.executemany("INSERT INTO main (text, level) VALUES (?, ?)", rows)
con.commit()
con.execute("vacuum")
```
