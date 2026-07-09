# SQL Appetizer

Simple defaults for taking a bite out of SQLite.

sql-appetizer offers a Rust API and a CLI for shell scripting or interactive use: `cargo install sql-appetizer`

## Log files

"Log" files are append-only arrays of text elements. They act like a Rust `Vec<(SystemTime, String)>`, but append-only.

```
# Append a new element to `log.db`, assuming it's a log file or doesn't exist yet
sql-appetizer log push log.db "Writing a new element to a file that may exist"

# Append multiple lines from stdin
echo "This will all\nbe one element." | sql-appetizer log push log.db

# Read back the elements as JSONL
sql-appetizer log iter log.db | jq .

# Get a single entry by its ID
sql-appetizer log get log.db 0

# Slice into the log file using a half-open interval
sql-appetizer log get log.db 0..2

# Slice into the log using a date interval
sql-appetizer log get log.db "2026-07-07 04:31:00+00:00..2026-07-07 04:32:00+00:00"
```

Every element has the Unix epoch and local time (as RFC 3339) from when it was written.

Use log files for app logging or personal journaling. SQLite offers more reliability than plaintext, and it keeps the timestamps out-of-band.

## KV files

"KV" files are key-value stores. They act like a Rust `BTreeMap<String, String>`.

```
# Insert an element into `my-settings.db`
sql-appetizer kv insert my-settings.db "/myapp/color_mode" "light_mode"

# Upsert the same element
sql-appetizer kv insert my-settings.db "/myapp/color_mode" "dark_mode"

# Return the most-recently-inserted value for the given key
sql-appetizer kv get my-settings.db "/myapp/color_mode"

# Returns non-zero if the key isn't present
sql-appetizer kv contains-key my-settings.db "/myapp/color_mode"

# Insert an empty string to use any KV file as a set
sql-appetizer kv insert my-settings.db "/myapp/bookmarks/https://example.com/" ""

# Get a range of elements matching a prefix
sql-appetizer kv with-prefix my-settings.db "/myapp/"

# Get all elements
sql-appetizer kv with-prefix my-settings.db
```

KV elements have no metadata.

Use KV files for app settings, bookmark storage, or keeping metadata about file paths. SQLite offers more reliability than plain JSON, and lookups are faster for large files.

_Finally, a Windows Registry for GNU/Linux._
