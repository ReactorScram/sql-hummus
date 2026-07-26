# sql-hummus

[docs.rs](https://docs.rs/sql-hummus/) | [crates.io](https://crates.io/crates/sql-hummus) | [GitHub](https://github.com/ReactorScram/sql-hummus)

sql-hummus is a SQLite appetizer made from [sql-peas](https://github.com/ReactorScram/sql-peas).

sql-hummus wraps SQLite databases into on-disk vectors or B-tree maps.

sql-hummus is a living cheatsheet for "I just need `Vec<String>` or `BTreeMap<String, String>` on disk real quick".

Install sql-hummus `cargo install sql-hummus` to get the CLI, or add the library to your project with `cargo add sql-hummus`

## Log files

> Finally, a harder way to do `echo $DATA >> log.txt`

"Log" files are append-only arrays of text elements. They act like a Rust `Vec<(SystemTime, String)>`, but append-only and persisted on disk.

```
# Append a new element to `log.db`, assuming it's a log file or doesn't exist yet
sql-hummus log diary.db push "Writing a new element, creating the DB file if needed"

# Append multiple lines from stdin
echo "This will all\nbe one element." | sql-hummus log diary.db push

# Read back the elements as JSONL
sql-hummus log diary.db iter | jq .

# Get a single entry by its ID
sql-hummus log diary.db get 1
```

Every element has the Unix epoch and local time (as RFC 3339) from when it was written.

Use log files for app logging or personal journaling. SQLite offers more reliability than plaintext, and it keeps the timestamps out-of-band.

## KV files

> Finally, a Windows Registry for GNU/Linux.

"KV" files are key-value stores. They act like a Rust `BTreeMap<String, String>`.

```
# Insert an element into `settings.db`
sql-hummus kv settings.db insert /myapp/color_mode light_mode

# Upsert the same element
sql-hummus kv settings.db insert /myapp/color_mode dark_mode

# Return the most-recently-inserted value for the given key
sql-hummus kv settings.db get /myapp/color_mode

# Returns non-zero if the key isn't present
sql-hummus kv settings.db  contains-key /myapp/color_mode

# Insert an empty string to use any KV file as a set
sql-hummus kv settings.db insert /myapp/bookmarks/https://example.com/ ""

# Get a range of elements matching a prefix
sql-hummus kv settings.db with-prefix /myapp/

# Get all elements
sql-hummus kv settings.db with-prefix
```

KV elements have no metadata.

Use KV files for app settings, bookmark storage, or keeping metadata about file paths. SQLite (via sql-hummus) offers more reliability than plain JSON, and lookups are faster for large files.

## Beyond sql-hummus

When your project outgrows sql-hummus, drop down to sql-peas to run your own SQLite queries. Every sql-hummus file is a normal SQLite database, so you can mi

## fput

`fput/github.com/ReactorScram/sql-hummus`
