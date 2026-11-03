//! A read-only reader of one table's rows in a SQLite database file, for the library the Comebacks
//! page downloads (`comeback`): the file's header, table b-tree pages (interior and leaf), the
//! record format and overflow chains (<https://www.sqlite.org/fileformat2.html>). Every offset is
//! checked and every page is visited once at most, so a damaged file is an error, never a panic or
//! an endless loop. UTF-8 databases only; indexes, WAL and free pages are never read.

use std::collections::HashSet;

/// One stored value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// A table's columns, as its `CREATE TABLE` names them, and its rows in rowid order.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    /// The index of column `name`, matched without regard to ASCII case, as SQLite matches it.
    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(name))
    }
}

/// The rows of table `name` in the database `file`.
pub fn table(file: &[u8], name: &str) -> Result<Table, String> {
    let database = Database::open(file)?;
    let schema = database.rows(1)?;
    let text = |row: &[Value], index: usize| match row.get(index) {
        Some(Value::Text(text)) => Some(text.clone()),
        _ => None,
    };
    let entry = schema
        .iter()
        .find(|(_, row)| {
            text(row, 0).as_deref() == Some("table")
                && text(row, 1).is_some_and(|table| table.eq_ignore_ascii_case(name))
        })
        .map(|(_, row)| row)
        .ok_or_else(|| format!("there is no table {name}"))?;
    let root = match entry.get(3) {
        Some(Value::Integer(page)) => u32::try_from(*page).map_err(|_| "a bad root page")?,
        _ => return Err("the table has no root page".into()),
    };
    let sql = text(entry, 4).ok_or("the table has no definition")?;
    let Definition {
        columns,
        rowid_alias,
    } = definition(&sql)?;
    let rows = database
        .rows(root)?
        .into_iter()
        .map(|(rowid, mut values)| {
            // A column added after a row was written is missing from the row's record.
            values.resize(columns.len().max(values.len()), Value::Null);
            values.truncate(columns.len());
            // An INTEGER PRIMARY KEY is the rowid; the record holds NULL in its place.
            if let Some(alias) = rowid_alias
                && values.get(alias) == Some(&Value::Null)
            {
                values[alias] = Value::Integer(rowid);
            }
            values
        })
        .collect();
    Ok(Table { columns, rows })
}

struct Database<'a> {
    file: &'a [u8],
    page_size: usize,
    /// The bytes of each page that hold the b-tree: the page size less the reserved bytes.
    usable: usize,
}

impl<'a> Database<'a> {
    fn open(file: &'a [u8]) -> Result<Self, String> {
        if file.len() < 100 || &file[..16] != b"SQLite format 3\0" {
            return Err("not a SQLite database".into());
        }
        let page_size = match u16::from_be_bytes([file[16], file[17]]) {
            1 => 65_536,
            size => usize::from(size),
        };
        if !page_size.is_power_of_two() || !(512..=65_536).contains(&page_size) {
            return Err("a bad page size".into());
        }
        let usable = page_size - usize::from(file[20]);
        if usable < 480 {
            return Err("too many reserved bytes".into());
        }
        if u32::from_be_bytes([file[56], file[57], file[58], file[59]]) != 1 {
            return Err("not a UTF-8 database".into());
        }
        Ok(Self {
            file,
            page_size,
            usable,
        })
    }

    fn page(&self, number: u32) -> Result<&'a [u8], String> {
        let index = usize::try_from(number)
            .ok()
            .and_then(|number| number.checked_sub(1))
            .ok_or("page 0")?;
        let start = index
            .checked_mul(self.page_size)
            .ok_or("a page past the end")?;
        self.file
            .get(start..start + self.page_size)
            .ok_or_else(|| format!("page {number} is past the end of the file"))
    }

    /// Every row of the table b-tree rooted at `root`, in rowid order, with its rowid.
    fn rows(&self, root: u32) -> Result<Vec<(i64, Vec<Value>)>, String> {
        let mut visited = HashSet::new();
        let mut rows = Vec::new();
        let mut stack = vec![root];
        while let Some(number) = stack.pop() {
            if !visited.insert(number) {
                return Err(format!("page {number} is reached twice"));
            }
            let page = self.page(number)?;
            let header = if number == 1 { 100 } else { 0 };
            let kind = *page.get(header).ok_or("a page without a header")?;
            let cells = usize::from(read_u16(page, header + 3)?);
            match kind {
                // A leaf: the rows themselves.
                0x0D => {
                    for cell in 0..cells {
                        let offset = usize::from(read_u16(page, header + 8 + 2 * cell)?);
                        rows.push(self.leaf_cell(page, offset, &mut visited)?);
                    }
                }
                // An interior page: each cell's child holds the rows up to its key, the right-most
                // pointer the rest. Pushed last first, so they come off the stack in order.
                0x05 => {
                    stack.push(read_u32(page, header + 8)?);
                    for cell in (0..cells).rev() {
                        let offset = usize::from(read_u16(page, header + 12 + 2 * cell)?);
                        stack.push(read_u32(page, offset)?);
                    }
                }
                other => return Err(format!("page {number} is not a table page ({other:#04x})")),
            }
        }
        Ok(rows)
    }

    /// The rowid and values of the leaf cell at `offset` of `page`, its overflow pages read.
    fn leaf_cell(
        &self,
        page: &[u8],
        offset: usize,
        visited: &mut HashSet<u32>,
    ) -> Result<(i64, Vec<Value>), String> {
        let (size, read) = varint(page, offset)?;
        let size = usize::try_from(size)
            .ok()
            .filter(|&size| size <= self.file.len())
            .ok_or("a row larger than the file")?;
        let (rowid, read_id) = varint(page, offset + read)?;
        let start = offset + read + read_id;
        let local = self.local_payload(size);
        let mut payload = page
            .get(start..start + local)
            .ok_or("a row past the end of its page")?
            .to_vec();
        if local < size {
            let mut next = read_u32(page, start + local)?;
            while payload.len() < size {
                if next == 0 {
                    return Err("an overflow chain that ends early".into());
                }
                if !visited.insert(next) {
                    return Err(format!("page {next} is reached twice"));
                }
                let overflow = self.page(next)?;
                next = read_u32(overflow, 0)?;
                let take = (size - payload.len()).min(self.usable - 4);
                payload
                    .extend_from_slice(overflow.get(4..4 + take).ok_or("a short overflow page")?);
            }
        }
        Ok((rowid as i64, record(&payload)?))
    }

    /// How much of a payload of `size` bytes a table leaf holds on its own page.
    fn local_payload(&self, size: usize) -> usize {
        let usable = self.usable;
        let most = usable - 35;
        if size <= most {
            return size;
        }
        let least = (usable - 12) * 32 / 255 - 23;
        let local = least + (size - least) % (usable - 4);
        if local <= most { local } else { least }
    }
}

/// The values of one record.
fn record(payload: &[u8]) -> Result<Vec<Value>, String> {
    let (header_size, mut at) = varint(payload, 0)?;
    let header_size = usize::try_from(header_size)
        .ok()
        .filter(|&size| size <= payload.len())
        .ok_or("a record header past its end")?;
    let mut types = Vec::new();
    while at < header_size {
        let (kind, read) = varint(payload, at)?;
        types.push(kind);
        at += read;
    }
    let mut body = header_size;
    let mut values = Vec::with_capacity(types.len());
    for kind in types {
        let size = match kind {
            0 | 8 | 9 => 0,
            1..=4 => kind as usize,
            5 => 6,
            6 | 7 => 8,
            10 | 11 => return Err("a reserved serial type".into()),
            _ => usize::try_from((kind - 12) / 2).map_err(|_| "a value too long")?,
        };
        let bytes = payload
            .get(body..body.checked_add(size).ok_or("a value too long")?)
            .ok_or("a value past the end of its record")?;
        body += size;
        values.push(match kind {
            0 => Value::Null,
            1..=6 => {
                // Big-endian two's complement, sign-extended from its first byte.
                let mut number = if bytes[0] & 0x80 != 0 { -1i64 } else { 0 };
                for &byte in bytes {
                    number = (number << 8) | i64::from(byte);
                }
                Value::Integer(number)
            }
            7 => Value::Real(f64::from_be_bytes(
                bytes.try_into().map_err(|_| "a short real")?,
            )),
            8 => Value::Integer(0),
            9 => Value::Integer(1),
            kind if kind % 2 == 0 => Value::Blob(bytes.to_vec()),
            _ => Value::Text(
                String::from_utf8(bytes.to_vec()).map_err(|_| "text that is not UTF-8")?,
            ),
        });
    }
    Ok(values)
}

/// A variable-length integer at `at`, and the bytes it took: up to eight of seven bits each, a
/// ninth of eight.
fn varint(bytes: &[u8], at: usize) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    for index in 0..9 {
        let byte = *bytes.get(at + index).ok_or("a number past the end")?;
        if index == 8 {
            return Ok(((value << 8) | u64::from(byte), 9));
        }
        value = (value << 7) | u64::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    unreachable!("the ninth byte returns")
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    let pair = bytes.get(at..at + 2).ok_or("a number past the end")?;
    Ok(u16::from_be_bytes([pair[0], pair[1]]))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    let four = bytes.get(at..at + 4).ok_or("a number past the end")?;
    Ok(u32::from_be_bytes([four[0], four[1], four[2], four[3]]))
}

/// What a `CREATE TABLE` says the reader needs: the columns in order, and which one, if any, is
/// the rowid (an `INTEGER PRIMARY KEY`).
struct Definition {
    columns: Vec<String>,
    rowid_alias: Option<usize>,
}

fn definition(sql: &str) -> Result<Definition, String> {
    let open = sql.find('(').ok_or("a table definition without columns")?;
    let close = sql
        .rfind(')')
        .filter(|&close| close > open)
        .ok_or("an unclosed definition")?;
    let mut columns = Vec::new();
    let mut rowid_alias = None;
    for item in split_top_level(&sql[open + 1..close]) {
        let mut words = item.split_whitespace();
        let Some(first) = words.next() else { continue };
        let constraint = ["constraint", "primary", "unique", "check", "foreign"]
            .iter()
            .any(|keyword| first.eq_ignore_ascii_case(keyword));
        if constraint {
            continue;
        }
        let rest = item[item.find(first).unwrap_or(0) + first.len()..].to_ascii_lowercase();
        let declared = rest.split_whitespace().next().unwrap_or_default();
        if declared == "integer" && rest.contains("primary key") {
            rowid_alias = Some(columns.len());
        }
        columns.push(
            first
                .trim_matches(|character| matches!(character, '"' | '`' | '\'' | '[' | ']'))
                .to_owned(),
        );
    }
    if columns.is_empty() {
        return Err("a table without columns".into());
    }
    Ok(Definition {
        columns,
        rowid_alias,
    })
}

/// `text` split at its commas outside brackets and quotes.
fn split_top_level(text: &str) -> Vec<&str> {
    let (mut items, mut depth, mut quote, mut start) = (Vec::new(), 0usize, None, 0);
    for (index, character) in text.char_indices() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => quote = Some(character),
            (None, '[') => quote = Some(']'),
            (None, '(') => depth += 1,
            (None, ')') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                items.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(text[start..].trim());
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database SQLite itself wrote (`testdata/README.md`): table `main` as the library has it,
    /// page size 1024, 140 rows over interior and leaf pages, and one row of 1500 characters that
    /// spills onto overflow pages.
    const FIXTURE: &[u8] = include_bytes!("testdata/comeback-library.db");

    fn text(value: &Value) -> &str {
        match value {
            Value::Text(text) => text,
            other => panic!("not text: {other:?}"),
        }
    }

    #[test]
    fn every_row_reads_in_order_across_interior_and_overflow_pages() {
        let table = table(FIXTURE, "main").unwrap();
        assert_eq!(table.columns, ["id", "text", "level"]);
        assert_eq!(table.column("TEXT"), Some(1));
        assert_eq!(table.rows.len(), 140);
        for (index, row) in table.rows.iter().enumerate() {
            assert_eq!(
                row[0],
                Value::Integer(index as i64 + 1),
                "the rowid is the id"
            );
        }
        assert_eq!(text(&table.rows[0][1]), "line 1");
        assert_eq!(text(&table.rows[0][2]), "min");
        assert_eq!(text(&table.rows[1][2]), "max");
        assert_eq!(text(&table.rows[119][1]), "line 120");
        let long = table
            .rows
            .iter()
            .map(|row| text(&row[1]))
            .find(|line| line.len() > 1000);
        assert_eq!(
            long,
            Some("长".repeat(1500).as_str()),
            "read whole from its overflow pages"
        );
    }

    #[test]
    fn a_damaged_or_foreign_file_is_an_error() {
        assert!(table(FIXTURE, "nope").unwrap_err().contains("no table"));
        assert!(table(&FIXTURE[..2048], "main").is_err(), "cut short");
        assert!(
            table(
                b"not a database at all, not even close to one hundred bytes long",
                "main"
            )
            .is_err()
        );
        assert!(table(&[0u8; 4096], "main").is_err());
        // Point the table's root page at itself as its own right-most child.
        let schema = Database::open(FIXTURE).unwrap().rows(1).unwrap();
        let Value::Integer(root) = schema[0].1[3] else {
            panic!("a root page")
        };
        let start = (root as usize - 1) * 1024;
        let mut looped = FIXTURE.to_vec();
        assert_eq!(
            looped[start], 0x05,
            "the fixture's root is an interior page"
        );
        looped[start + 8..start + 12].copy_from_slice(&(root as u32).to_be_bytes());
        assert!(table(&looped, "main").unwrap_err().contains("twice"));
    }

    #[test]
    fn a_definition_names_its_columns_and_its_rowid() {
        let definition = definition(
            "CREATE TABLE main (id integer PRIMARY KEY NOT NULL, text varchar (255), level varchar (255) DEFAULT ('max'))",
        )
        .unwrap();
        assert_eq!(definition.columns, ["id", "text", "level"]);
        assert_eq!(definition.rowid_alias, Some(0));
        let quoted = definition_of("CREATE TABLE t (\"a,b\" TEXT, [c] INT, PRIMARY KEY (c))");
        assert_eq!(quoted, ["a,b", "c"]);
    }

    fn definition_of(sql: &str) -> Vec<String> {
        definition(sql).unwrap().columns
    }

    #[test]
    fn numbers_read_as_sqlite_writes_them() {
        assert_eq!(varint(&[0x7F], 0).unwrap(), (127, 1));
        assert_eq!(varint(&[0x81, 0x00], 0).unwrap(), (128, 2));
        assert_eq!(varint(&[0xFF; 9], 0).unwrap(), (u64::MAX, 9));
        assert!(varint(&[0x81], 0).is_err());
        // header 4: integer of 1 byte (-1), of 2 bytes (300), text of 2 bytes "hi".
        let values = record(&[4, 1, 2, 17, 0xFF, 0x01, 0x2C, b'h', b'i']).unwrap();
        assert_eq!(
            values,
            [
                Value::Integer(-1),
                Value::Integer(300),
                Value::Text("hi".into())
            ]
        );
        assert!(record(&[2, 17, b'h']).is_err(), "text past the end");
    }
}
