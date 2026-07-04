# Dataframe convert

## `dataframe_convert`

    $ dataframe_convert --help
    Examples:
    
    $ dataframe-convert metadata input.csv (view metadata) (notice that the column which should be date is inferred as string due to not matching the default date-format)
    
    $ dataframe-convert metadata --column col_name=date:ifmt=%m/%d/%Y input.csv (now the schema looks correct)
    
    $ dataframe-convert convert input.csv output.parquet (now we have a parquet file)
    
    $ dataframe-convert metadata output.parquet (look at how many bytes we saved for each column)
    
    We read dataframes lazily, where possible, so this is suitable for large amounts of data.
    
    More complex operations than light serialization/deserialization of primitive types, concatenatation, converting dataframe formats, are out-of-scope. I suggest using duckdb's excellent CLI, e.g.:
    
    duckdb -c "SELECT C, AVG(D) FROM read_csv_auto('path/to/file.csv') GROUP BY C;"
    
    Usage: dataframe_convert <COMMAND>
    
    Commands:
      cat       Concat input dataframes and convert to output format. Silent on success
      metadata  Print metadata (schema and summary statistics) of input dataframes
      help      Print this message or the help of the given subcommand(s)
    
    Options:
      -h, --help
              Print help (see a summary with '-h')    # END

## `dataframe_convert metadata`

    $ dataframe_convert metadata --help
    Print metadata (schema and summary statistics) of input dataframes.
    
    Example:
    
    $ dataframe_convert metadata data/sample.csv source: data/sample.csv file_size: 358 mem_size: 162 overhead: 2.2098765432098766 parse_secs: 0.02313957 n_rows: 5 columns: - name: id dtype: u8 count: 5 non_null_count: 5 mem_size: 5 dtype_specific_meta: kind: numeric mean: 3.0 stddev: 1.5811388300841898 quantiles: min: 1.0 25%: 2.0 50%: 3.0 75%: 4.0 max: 5.0 - name: name dtype: str count: 5 non_null_count: 5 mem_size: 19 dtype_specific_meta: kind: categorical n_unique: 5 most_common: Alice: 1 Bob: 1 Carol: 1 - name: score dtype: f64 count: 5 non_null_count: 5 mem_size: 40 dtype_specific_meta: kind: numeric mean: 78.52000000000001 stddev: 20.63545492592785 quantiles: min: 45.1 25%: 72.3 50%: 88.7 75%: 91.0 max: 95.5 - name: active dtype: bool count: 5 non_null_count: 5 mem_size: 1 dtype_specific_meta: kind: categorical n_unique: 2 most_common: 'true': 3 'false': 2 - name: birth_date dtype: date count: 5 non_null_count: 5 mem_size: 20 dtype_specific_meta: kind: datetime unit: ms tz: null - name: created_at dtype: datetime[μs] count: 5 non_null_count: 5 mem_size: 40 dtype_specific_meta: kind: datetime unit: us tz: null - name: session_ms dtype: u16 count: 5 non_null_count: 5 mem_size: 10 dtype_specific_meta: kind: numeric mean: 13900.0 stddev: 17887.70527485289 quantiles: min: 300.0 25%: 5000.0 50%: 7200.0 75%: 12000.0 max: 45000.0 - name: role dtype: str count: 5 non_null_count: 5 mem_size: 27 dtype_specific_meta: kind: categorical n_unique: 3 most_common: admin: 2 user: 2 moderator: 1///     # END
    
    Usage: dataframe_convert metadata [OPTIONS] [PATHS]...
    
    Arguments:
      [PATHS]...
              
    
    Options:
      -i, --input-format <INPUT_FORMAT>
              input_format will be inferred if not given.
              
              Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, XLS, ODS, SQLite, DuckDB
              
              Supports additional args like `xlsx:flag,key=val`:
              
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
              
              - duration takes `unit=unit_str`, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Duration columns are de/serialized natively for formats that support them (Parquet, IPC); for CSV, JSON, etc. they are de/serialized as integers. For example `dur=duration:unit=ms`.
              
              - datetime also takes unit=unit_str, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Internally, Polars will use an integer number of these units.
              
              - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format. See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>
    
      -N, --no-infer
              Skip automatic dtype inference for unspecified columns
    
          --format <FORMAT>
              [default: yaml]
    
      -h, --help
              Print help (see a summary with '-h')    # END
    
    Usage: dataframe_convert metadata [OPTIONS] [PATHS]...
    
    Arguments:
      [PATHS]...
              
    
    Options:
      -i, --input-format <INPUT_FORMAT>
              input_format will be inferred if not given.
              
              Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, XLS, ODS, SQLite, DuckDB
              
              Supports additional args like `xlsx:flag,key=val`:
              
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
              
              - duration takes `unit=unit_str`, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Duration columns are de/serialized natively for formats that support them (Parquet, IPC); for CSV, JSON, etc. they are de/serialized as integers. For example `dur=duration:unit=ms`.
              
              - datetime also takes unit=unit_str, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Internally, Polars will use an integer number of these units.
              
              - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format. See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>
    
      -N, --no-infer
              Skip automatic dtype inference for unspecified columns
    
          --format <FORMAT>
              [default: yaml]
    
      -h, --help
              Print help (see a summary with '-h')

    # END

## `dataframe_convert cat`

    $ dataframe_convert cat --help
    Concat input dataframes and convert to output format. Silent on success.
    
    Dtypes are automatically inferred for columns not specified via --dtypes; pass --no-infer to disable.
    
    Examples:
    
    $ dataframe_convert cat a.csv b.csv out.parquet $ dataframe_convert cat --no-infer --dtypes id=int data.json out.parquet
    
    All inputs must be in the same format and same schema.
    
    Usage: dataframe_convert cat [OPTIONS] [PATHS] [PATHS]...
    
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
              
              Supports additional args like `xlsx:flag,key=val`:
              
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
              
              - duration takes `unit=unit_str`, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Duration columns are de/serialized natively for formats that support them (Parquet, IPC); for CSV, JSON, etc. they are de/serialized as integers. For example `dur=duration:unit=ms`.
              
              - datetime also takes unit=unit_str, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Internally, Polars will use an integer number of these units.
              
              - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format. See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>
    
      -N, --no-infer
              Skip automatic dtype inference for unspecified columns
    
      -h, --help
              Print help (see a summary with '-h')    # END
    
    Usage: dataframe_convert metadata [OPTIONS] [PATHS]...
    
    Arguments:
      [PATHS]...
              
    
    Options:
      -i, --input-format <INPUT_FORMAT>
              input_format will be inferred if not given.
              
              Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, XLS, ODS, SQLite, DuckDB
              
              Supports additional args like `xlsx:flag,key=val`:
              
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
              
              - duration takes `unit=unit_str`, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Duration columns are de/serialized natively for formats that support them (Parquet, IPC); for CSV, JSON, etc. they are de/serialized as integers. For example `dur=duration:unit=ms`.
              
              - datetime also takes unit=unit_str, where unit_str is ns, nano, nanos, nanoseconds, (similar for micros), (similar for millis). Internally, Polars will use an integer number of these units.
              
              - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format. See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>
    
      -N, --no-infer
              Skip automatic dtype inference for unspecified columns
    
          --format <FORMAT>
              [default: yaml]
    
      -h, --help
              Print help (see a summary with '-h')

    # END

## `dataframe_convert cat`

    $ dataframe_convert cat --help
    # END
