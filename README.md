# dataframe_convert

Convert dataframes between different formats.

## Example

<!-- BEGIN GEN -->
```sh
$ dataframe_convert metadata --dtypes id=int --dtypes name=str --dtypes score=float --dtypes active=bool --dtypes birth_date=date:ifmt=%Y-%m-%d --dtypes created_at=datetime:ifmt=%Y-%m-%dT%H:%M:%S --dtypes session_ms=duration:from_int,to_int,unit=ms --dtypes role=cat data/sample.csv
source: data/sample.csv
file_size: 358
mem_size: 200
overhead: 1.79
parse_secs: 0.008510955
n_rows: 5
columns:
- name: id
  dtype: i64
  count: 5
  non_null_count: 5
  mem_size: 40
  dtype_specific_meta:
    kind: numeric
    mean: 3.0
    stddev: 1.5811388300841898
    quantiles:
    - - 0.0
      - 1.0
    - - 0.25
      - 2.0
    - - 0.5
      - 3.0
    - - 0.75
      - 4.0
    - - 1.0
      - 5.0
- name: name
  dtype: str
  count: 5
  non_null_count: 5
  mem_size: 19
  dtype_specific_meta:
    kind: categorical
    n_unique: 5
    n_most_common:
    - - '"Alice"'
      - 1
    - - '"Bob"'
      - 1
    - - '"Carol"'
      - 1
- name: score
  dtype: f32
  count: 5
  non_null_count: 5
  mem_size: 20
  dtype_specific_meta:
    kind: numeric
    mean: 78.51999969482422
    stddev: 20.635454937389376
    quantiles:
    - - 0.0
      - 45.099998474121094
    - - 0.25
      - 72.30000305175781
    - - 0.5
      - 88.69999694824219
    - - 0.75
      - 91.0
    - - 1.0
      - 95.5
- name: active
  dtype: bool
  count: 5
  non_null_count: 5
  mem_size: 1
  dtype_specific_meta:
    kind: categorical
    n_unique: 2
    n_most_common:
    - - 'true'
      - 3
    - - 'false'
      - 2
- name: birth_date
  dtype: date
  count: 5
  non_null_count: 5
  mem_size: 20
  dtype_specific_meta:
    kind: datetime
    unit: ms
    tz: null
- name: created_at
  dtype: datetime[μs]
  count: 5
  non_null_count: 5
  mem_size: 40
  dtype_specific_meta:
    kind: datetime
    unit: us
    tz: null
- name: session_ms
  dtype: duration[ms]
  count: 5
  non_null_count: 5
  mem_size: 40
  dtype_specific_meta:
    kind: duration
    min:
      secs: 0
      nanos: 0
    max:
      secs: 0
      nanos: 0
    unit: ms
- name: role
  dtype: cat
  count: 5
  non_null_count: 5
  mem_size: 20
  dtype_specific_meta:
    kind: categorical
    n_unique: 3
    n_most_common:
    - - '"admin"'
      - 2
    - - '"user"'
      - 2
    - - '"moderator"'
      - 1
```
<!-- END GEN -->

## Install

```sh
nix run github:user/dataframe_convert -- --help
```

Or build from source:

```sh
cargo build --release
```

## CLI Reference

<!-- BEGIN HELP -->
```sh
$ dataframe_convert --help
``` $ dataframe-convert metadata input.csv (view metadata) (notice that the column which should be date is inferred as string due to not matching the default date-format)

$ dataframe-convert metadata --column col_name=date:ifmt=%m/%d/%Y input.csv (now the schema looks correct)

$ dataframe-convert convert input.csv output.parquet (now we have a parquet file)

$ dataframe-convert metadata output.parquet (look at how many bytes we saved for each column) ```

We read dataframes lazily, where possible, so this is suitable for large amounts of data.

More complex operations than light serialization/deserialization of primitive types, concatenatation, converting dataframe formats, are out-of-scope. I suggest using duckdb's excellent CLI, e.g.:

``` duckdb -c "SELECT C, AVG(D) FROM read_csv_auto('path/to/file.csv') GROUP BY C;" ```

Usage: dataframe_convert <COMMAND>

Commands:
  convert   Concat all input dataframes, convert to output dataframe format
  metadata  Print metadata, including schema and summary statistics, of the input dataframes
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')
```
<!-- END HELP -->

### cat

<!-- BEGIN HELP CAT -->
```sh
$ dataframe_convert convert --help
Concat all input dataframes, convert to output dataframe format.

All inputs must be in the same format and same schema.

Usage: dataframe_convert convert [OPTIONS] [PATHS] [PATHS]...

Arguments:
  [PATHS] [PATHS]...
          N>0 input paths followed by 1 output path

Options:
  -o, --output-format <OUTPUT_FORMAT>
          output_format will be inferred if not given.
          
          Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, SQLite, DuckDB, MD/Markdown,
          
          [default: ""]

  -i, --input-format <INPUT_FORMAT>
          input_format will be inferred if not given.
          
          Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, XLS, ODS, SQLite, DuckDB
          
          Supports additional args like `xlsx:flag1,flag2,key1=val1,key2=val2`:
          
          - Spreadsheet types (`csv`, `xlsx`, `xls`, `ods`) support `no_header` flag
          
          - Spreadsheet types support `ignore_errors` flag, which replaces deserialization failures with NULL.
          
          - Spreadsheet types supports the key `skip_rows=n`, which skips the first n rows.
          
          - Database types (sqlite, duckdb) support `table=name`, which reads from the table named `name`.
          
          [default: ""]

      --dtypes <COLUMN>
          Specification like: `col_name=type_name`
          
          We make a best effort to infer the most precise schema, but providing a schema makes the tool more precise, especially when using a weakly typed format like CSV.
          
          Give multiple times to specify multiple columns.
          
          Supported type_names:
          
          - str = string = utf8
          
          - blob = bin = binary = bytes
          
          - i8 = int8, u8 = uint8, up to u128; int = integer = int32
          
          - f16 = float16 up to 64, float = f32, double = f64
          
          - b = bool = boolean
          
          - date, time, duration = timedelta, datetime = dt
          
          - cat = categorical, which are strings "interned" as integers
          
          Some typs take optional arguments, like `type_name:arg1,arg2`. Supported conversion arguments:
          
          - str:strip
          
          - str:max_size
          
          - date, time, and datetime take `ifmt=fmt_string` and `ofmt=fmt_string` where fmt_string is a Chrono strptime/stftime string. For example `date_col=date:ifmt=%Y-%M-%d`. ifmt influences how the column is read, whereas ofmt changes how it is written. Commas and backslashes may be escaped by backslashes.
          
          - duration takes `from_int` and `to_int` which indicate that the duration column should be read/written as an integer (in the given time unit). For example `dur=duration:from_int,to_int,unit=ms`.
          
          - duration and datetime also takes unit=unit_str, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Internally, Polars will use an integer number of these units.
          
          - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format. See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>

  -h, --help
          Print help (see a summary with '-h')
```
<!-- END HELP CAT -->

### metadata

<!-- BEGIN HELP METADATA -->
```sh
$ dataframe_convert metadata --help
Print metadata, including schema and summary statistics, of the input dataframes

Usage: dataframe_convert metadata [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...
          

Options:
  -i, --input-format <INPUT_FORMAT>
          input_format will be inferred if not given.
          
          Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, XLS, ODS, SQLite, DuckDB
          
          Supports additional args like `xlsx:flag1,flag2,key1=val1,key2=val2`:
          
          - Spreadsheet types (`csv`, `xlsx`, `xls`, `ods`) support `no_header` flag
          
          - Spreadsheet types support `ignore_errors` flag, which replaces deserialization failures with NULL.
          
          - Spreadsheet types supports the key `skip_rows=n`, which skips the first n rows.
          
          - Database types (sqlite, duckdb) support `table=name`, which reads from the table named `name`.
          
          [default: ""]

      --dtypes <COLUMN>
          Specification like: `col_name=type_name`
          
          We make a best effort to infer the most precise schema, but providing a schema makes the tool more precise, especially when using a weakly typed format like CSV.
          
          Give multiple times to specify multiple columns.
          
          Supported type_names:
          
          - str = string = utf8
          
          - blob = bin = binary = bytes
          
          - i8 = int8, u8 = uint8, up to u128; int = integer = int32
          
          - f16 = float16 up to 64, float = f32, double = f64
          
          - b = bool = boolean
          
          - date, time, duration = timedelta, datetime = dt
          
          - cat = categorical, which are strings "interned" as integers
          
          Some typs take optional arguments, like `type_name:arg1,arg2`. Supported conversion arguments:
          
          - str:strip
          
          - str:max_size
          
          - date, time, and datetime take `ifmt=fmt_string` and `ofmt=fmt_string` where fmt_string is a Chrono strptime/stftime string. For example `date_col=date:ifmt=%Y-%M-%d`. ifmt influences how the column is read, whereas ofmt changes how it is written. Commas and backslashes may be escaped by backslashes.
          
          - duration takes `from_int` and `to_int` which indicate that the duration column should be read/written as an integer (in the given time unit). For example `dur=duration:from_int,to_int,unit=ms`.
          
          - duration and datetime also takes unit=unit_str, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Internally, Polars will use an integer number of these units.
          
          - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format. See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>

      --format <FORMAT>
          [default: yaml]

  -h, --help
          Print help (see a summary with '-h')
```
<!-- END HELP METADATA -->
