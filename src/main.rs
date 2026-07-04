use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use dataframe_convert::datatype_ser::datatype_ser_to_schema;
use dataframe_convert::infer::{DateLocale, infer_df};
use dataframe_convert::{InputFormat, OutputFormat};

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.markdown_help {
        clap_markdown::print_help_markdown::<Cli>();
        return Ok(())
    }
    match cli.command {
        Command::Cat(args) => {
            let (inputs, output) = args.paths.split_at(args.paths.len() - 1);
            let output = &output[0];
            let out_fmt = if args.output_format.is_empty() {
                infer_output_format(output)?
            } else {
                interpret_output_format(&args.output_format)?
            };
            let in_fmt = if args.shared.input_format.is_empty() {
                infer_input_format(&inputs[0])?
            } else {
                interpret_input_format(&args.shared.input_format)?
            };
            let mut expanded_inputs: Vec<std::path::PathBuf> = Vec::new();
            for input in inputs {
                expanded_inputs.extend(expand_input_path(input)?);
            }
            let pl_inputs: Vec<_> = expanded_inputs
                .iter()
                .filter_map(|p| p.to_str())
                .map(|s| strip_colon(s).into())
                .collect();
            let dfs = dataframe_convert::read::read_lfs(in_fmt.clone(), &pl_inputs, &args.shared.column)
                .context("setting up reader")?;
            let lf = dataframe_convert::concat_lf_diagonal(&dfs).context("concatenating")?;
            let lf = if !args.shared.no_infer {
                let schema = datatype_ser_to_schema(&args.shared.column, &in_fmt);
                infer_df(lf, &schema, DateLocale::Auto)
            } else {
                lf
            };
            dataframe_convert::write::write_lf(
                lf,
                out_fmt,
                strip_colon(output.to_str().unwrap()).into(),
                &args.shared.column,
            )
            .context("writing")?;
        }
        Command::Metadata(args) => {
            let in_fmt = if args.shared.input_format.is_empty() {
                infer_input_format(&args.paths[0])?
            } else {
                interpret_input_format(&args.shared.input_format)?
            };
            let mut expanded_paths: Vec<std::path::PathBuf> = Vec::new();
            for path in &args.paths {
                expanded_paths.extend(expand_input_path(path)?);
            }
            for path in &expanded_paths {
                let start = std::time::Instant::now();
                let pl_path: polars::prelude::PlRefPath =
                    strip_colon(path.to_str().context("non-utf8 path")?).into();
                let lf =
                    dataframe_convert::read::read_lf(in_fmt.clone(), pl_path, &args.shared.column)
                        .context("setting up reader")?;
                let lf = if !args.shared.no_infer {
                    let schema = datatype_ser_to_schema(&args.shared.column, &in_fmt);
                    infer_df(lf, &schema, DateLocale::Auto)
                } else {
                    lf
                };
                let df = lf.collect().context("lf->df")?;
                let end = std::time::Instant::now();
                let path_str = path.display().to_string();
                let file_size = std::fs::metadata(strip_colon(&path_str))
                    .map(|m| m.len())
                    .unwrap_or(0);
                let meta = dataframe_convert::metadata::metadata_frame(
                    df,
                    path_str,
                    (end - start).as_secs_f64(),
                    file_size,
                );
                let s = serde_yaml::to_string(&meta)?;
                println!("{s}");
            }
        }
    }
    Ok(())
}

fn colon_split(path: &std::path::Path) -> (String, Option<String>) {
    let s = path.to_str().unwrap_or("");
    if let Some(pos) = s.rfind(':') {
        let ext = std::path::Path::new(&s[..pos])
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        (ext, Some(s[pos + 1..].to_string()))
    } else {
        (
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default(),
            None,
        )
    }
}

fn strip_colon(s: &str) -> &str {
    s.rfind(':').map(|pos| &s[..pos]).unwrap_or(s)
}

fn infer_input_format(path: &std::path::PathBuf) -> Result<InputFormat> {
    let (ext, subresource) = colon_split(path);
    let stem_lower = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" => Ok(InputFormat::Csv {
            separator: b',',
            has_header: true,
            ignore_errors: false,
            skip_rows: 0,
        }),
        "tsv" => Ok(InputFormat::Csv {
            separator: b'\t',
            has_header: true,
            ignore_errors: false,
            skip_rows: 0,
        }),
        "parquet" | "pq" => Ok(InputFormat::Parquet),
        "json" | "ndjson" | "jsonl" => Ok(InputFormat::Json {
            ignore_errors: false,
        }),
        "ipc" | "arrow" | "feather" => Ok(InputFormat::Ipc),
        "xlsx" => Ok(InputFormat::Xlsx {
            has_header: true,
            ignore_errors: false,
        }),
        "xls" => Ok(InputFormat::Xls {
            has_header: true,
            ignore_errors: false,
        }),
        "ods" => Ok(InputFormat::Ods {
            has_header: true,
            ignore_errors: false,
        }),
        "sqlite" | "sqlite3" | "db" | "duckdb" => {
            let table = subresource.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("data")
                    .to_string()
            });
            if ext == "duckdb" || ext.contains("duck") {
                Ok(InputFormat::Duckdb { table })
            } else {
                Ok(InputFormat::Sqlite {
                    table,
                    ignore_errors: false,
                })
            }
        }
        _ => {
            if stem_lower.ends_with(".sqlite") || stem_lower.ends_with(".sqlite3") {
                Ok(InputFormat::Sqlite {
                    table: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("data")
                        .to_string(),
                    ignore_errors: false,
                })
            } else {
                bail!("could not determine input format for {path:?}")
            }
        }
    }
}

fn infer_output_format(path: &std::path::PathBuf) -> Result<OutputFormat> {
    let (ext, subresource) = colon_split(path);
    let stem_lower = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" => Ok(OutputFormat::Csv {
            separator: b',',
            emit_header: true,
        }),
        "tsv" => Ok(OutputFormat::Csv {
            separator: b'\t',
            emit_header: true,
        }),
        "parquet" | "pq" => Ok(OutputFormat::Parquet),
        "json" | "ndjson" | "jsonl" => Ok(OutputFormat::Json),
        "ipc" | "arrow" | "feather" => Ok(OutputFormat::Ipc),
        "xlsx" => Ok(OutputFormat::Xlsx { emit_header: true }),
        "md" | "markdown" => Ok(OutputFormat::Md),
        "sqlite" | "sqlite3" | "db" | "duckdb" => {
            let table = subresource.unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("data")
                    .to_string()
            });
            if ext == "duckdb" || ext.contains("duck") {
                Ok(OutputFormat::Duckdb { table })
            } else {
                Ok(OutputFormat::Sqlite { table })
            }
        }
        _ => {
            if stem_lower.ends_with(".sqlite") || stem_lower.ends_with(".sqlite3") {
                Ok(OutputFormat::Sqlite {
                    table: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("data")
                        .to_string(),
                })
            } else {
                bail!("could not determine output format for {path:?}")
            }
        }
    }
}

fn interpret_input_format(format: &str) -> Result<InputFormat> {
    let (fmt_name, args_str) = format.split_once(':').unwrap_or((format, ""));
    let args = split_escaped(args_str);
    let fmt_lower = fmt_name.to_lowercase();

    let has_header = !args.iter().any(|a| a == "no_header");
    let ignore_errors = args.iter().any(|a| a == "ignore_errors");
    let skip_rows = find_arg(&args, "skip_rows")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let table = find_arg(&args, "table").unwrap_or_else(|| "data".into());

    let sep_str = find_arg(&args, "sep");
    let separator = match sep_str {
        Some(ref s) => parse_separator(s)?,
        None => b',',
    };

    Ok(match fmt_lower.as_str() {
        "csv" => InputFormat::Csv {
            separator,
            has_header,
            ignore_errors,
            skip_rows,
        },
        "tsv" => InputFormat::Csv {
            separator: b'\t',
            has_header,
            ignore_errors,
            skip_rows,
        },
        "parquet" | "pq" => InputFormat::Parquet,
        "json" | "ndjson" | "jsonl" => InputFormat::Json { ignore_errors },
        "ipc" | "arrow" | "feather" => InputFormat::Ipc,
        "xlsx" => InputFormat::Xlsx {
            has_header,
            ignore_errors,
        },
        "xls" => InputFormat::Xls {
            has_header,
            ignore_errors,
        },
        "ods" => InputFormat::Ods {
            has_header,
            ignore_errors,
        },
        "sqlite" | "sqlite3" | "db" => InputFormat::Sqlite {
            table,
            ignore_errors,
        },
        "duckdb" => InputFormat::Duckdb { table },
        _ => bail!("unknown input format: {fmt_name}"),
    })
}

fn interpret_output_format(format: &str) -> Result<OutputFormat> {
    let (fmt_name, args_str) = format.split_once(':').unwrap_or((format, ""));
    let args = split_escaped(args_str);
    let fmt_lower = fmt_name.to_lowercase();

    let emit_header = !args.iter().any(|a| a == "no_header");
    let table = find_arg(&args, "table").unwrap_or_else(|| "data".into());

    let sep_str = find_arg(&args, "sep");
    let separator = match sep_str {
        Some(ref s) => {
            if s.len() == 1 {
                s.as_bytes()[0]
            } else {
                bail!("invalid separator: {s:?} (expected a single character)")
            }
        }
        None => b',',
    };

    Ok(match fmt_lower.as_str() {
        "csv" => OutputFormat::Csv {
            separator,
            emit_header,
        },
        "tsv" => OutputFormat::Csv {
            separator: b'\t',
            emit_header,
        },
        "parquet" | "pq" => OutputFormat::Parquet,
        "json" | "ndjson" | "jsonl" => OutputFormat::Json,
        "ipc" | "arrow" | "feather" => OutputFormat::Ipc,
        "xlsx" => OutputFormat::Xlsx { emit_header },
        "sqlite" | "sqlite3" | "db" => OutputFormat::Sqlite { table },
        "duckdb" => OutputFormat::Duckdb { table },
        "md" | "markdown" => OutputFormat::Md,
        _ => bail!("unknown output format: {fmt_name}"),
    })
}

fn expand_input_path(path: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let s = path.to_str().context("non-utf8 path")?;
    let (base, suffix) = match s.rfind(':') {
        Some(pos) => (&s[..pos], &s[pos + 1..]),
        None => return Ok(vec![path.to_path_buf()]),
    };
    if suffix != "*" {
        return Ok(vec![path.to_path_buf()]);
    }

    let ext = std::path::Path::new(base)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let names: Vec<String> = match ext.as_str() {
        "xlsx" => dataframe_convert::read_excel::sheet_names_xlsx(base)?,
        "xls" => dataframe_convert::read_excel::sheet_names_xls(base)?,
        "ods" => dataframe_convert::read_excel::sheet_names_ods(base)?,
        _ => return Ok(vec![path.to_path_buf()]),
    };

    Ok(names
        .into_iter()
        .map(|name| {
            let mut p = std::path::PathBuf::from(base);
            p.set_extension(format!("{ext}:{name}"));
            p
        })
        .collect())
}

///
/// Examples:
/// 
///     $ dataframe-convert metadata input.csv
///     (view metadata)
///     (notice that the column which should be date is inferred as string due to not matching the default date-format)
///     
///     $ dataframe-convert metadata --column col_name=date:ifmt=%m/%d/%Y input.csv
///     (now the schema looks correct)
///     
///     $ dataframe-convert convert input.csv output.parquet
///     (now we have a parquet file)
///     
///     $ dataframe-convert metadata output.parquet
///     (look at how many bytes we saved for each column)
///
/// We read dataframes lazily, where possible, so this is suitable for large
/// amounts of data.
///
/// More complex operations than light serialization/deserialization of
/// primitive types, concatenatation, converting dataframe formats, are
/// out-of-scope. I suggest using duckdb's excellent CLI, e.g.:
///
///     duckdb -c "SELECT C, AVG(D) FROM read_csv_auto('path/to/file.csv') GROUP BY C;"
///
#[derive(Parser)]
#[command(name = "dataframe_convert", verbatim_doc_comment)]
struct Cli {
    #[arg(long, hide = true)]
    markdown_help: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Cat(CatArgs),
    Metadata(MetadataArgs),
}

#[derive(clap::Args)]
struct SharedOpts {
    /// input_format will be inferred if not given.
    ///
    /// Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, XLS, ODS, SQLite, DuckDB
    ///
    /// Supports additional args like `xlsx:flag,key=val`:
    ///
    /// - Spreadsheet types (`csv`, `xlsx`, `xls`, `ods`) support `no_header` flag
    ///
    /// - Spreadsheet types support `ignore_errors` flag, which replaces
    ///   deserialization failures with NULL.
    ///
    /// - Spreadsheet types supports the key `skip_rows=n`, which skips the
    ///   first n rows.
    ///
    /// - Database types (sqlite, duckdb) support `table=name`, which reads from
    ///   the table named `name`.
    #[arg(short, long, default_value = "")]
    input_format: String,

    /// Specification like: `col_name=type_name`
    ///
    ///
    /// We make a best effort to infer the most precise schema, but providing a
    /// schema makes the tool more precise, especially when using a weakly typed
    /// format like CSV.
    ///
    /// Give multiple times to specify multiple columns.
    ///
    /// Supported type_names:
    ///
    /// - str = string = utf8
    ///
    /// - blob = bin = binary = bytes
    ///
    /// - i8 = int8, u8 = uint8, up to u128; int = integer = int32
    ///
    /// - f16 = float16 up to 64, float = f32, double = f64
    ///
    /// - b = bool = boolean
    ///
    /// - date, time, duration = timedelta, datetime = dt
    ///
    /// - cat = categorical, which are strings "interned" as integers
    ///
    /// Some typs take optional arguments, like `type_name:arg1,arg2`. Supported
    /// conversion arguments:
    ///
    /// - str:strip
    ///
    /// - str:max_size
    ///
    /// - date, time, and datetime take `ifmt=fmt_string` and
    ///   `ofmt=fmt_string` where fmt_string is a Chrono strptime/stftime
    ///   string. For example `date_col=date:ifmt=%Y-%M-%d`. ifmt influences how
    ///   the column is read, whereas ofmt changes how it is written. Commas and
    ///   backslashes may be escaped by backslashes.
    ///
    /// - duration takes `unit=unit_str`, where unit_str is ns, nano, nanos,
    ///   nanoseconds, (similar for micros), (similar for millis). Duration
    ///   columns are de/serialized natively for formats that support them
    ///   (Parquet, IPC); for CSV, JSON, etc. they are de/serialized as integers.
    ///   For example `dur=duration:unit=ms`.
    ///
    /// - datetime also takes unit=unit_str, where unit_str is ns, nano, nanos,
    ///   nanoseconds, (similar for micros), (similar for millis). Internally,
    ///   Polars will use an integer number of these units.
    ///
    /// - datetime:tz=tz_str, where tz_str is `UTC` or `Area/Location` format.
    ///   See <https://en.wikipedia.org/wiki/List_of_tz_database_time_zones>
    #[arg(long = "dtypes", value_parser = parse_col_spec)]
    column: Vec<(String, dataframe_convert::DataTypeSer)>,

    /// Skip automatic dtype inference for unspecified columns.
    #[arg(short = 'N', long)]
    no_infer: bool,
}

/// Concat input dataframes and convert to output format. Silent on success.
///
/// Dtypes are automatically inferred for columns not specified via --dtypes;
/// pass --no-infer to disable.
///
/// Examples:
///
///     $ dataframe_convert cat a.csv b.csv out.parquet
///     $ dataframe_convert cat --no-infer --dtypes id=int data.json out.parquet
///
/// All inputs must be in the same format and same schema.
#[derive(clap::Args)]
#[command(verbatim_doc_comment)]
struct CatArgs {
    /// output_format will be inferred if not given.
    ///
    /// Supports: CSV/TSV, Parquet, JSON, IPC (arrow), XLSX, SQLite, DuckDB, MD/Markdown,
    #[arg(short, long, default_value = "")]
    output_format: String,

    #[command(flatten)]
    shared: SharedOpts,

    /// N>0 input paths followed by 1 output path.
    #[arg(num_args = 2..)]
    paths: Vec<std::path::PathBuf>,
}

/// Print metadata (schema and summary statistics) of input dataframes.
///
/// Example:
///
///     $ dataframe_convert metadata data/sample.csv
///     source: data/sample.csv
///     file_size: 358
///     mem_size: 162
///     overhead: 2.2098765432098766
///     parse_secs: 0.028144523
///     n_rows: 5
///     columns:
///     - name: id
///       dtype: u8
///       count: 5
///       non_null_count: 5
///       mem_size: 5
///       dtype_specific_meta:
///         kind: numeric
///         mean: 3.0
///         stddev: 1.5811388300841898
///         quantiles:
///           min: 1.0
///           25%: 2.0
///           50%: 3.0
///           75%: 4.0
///           max: 5.0
///     - name: name
///       dtype: str
///       count: 5
///       non_null_count: 5
///       mem_size: 19
///       dtype_specific_meta:
///         kind: categorical
///         n_unique: 5
///         most_common:
///           Alice: 1
///           Bob: 1
///           Carol: 1
///     - name: score
///       dtype: f64
///       count: 5
///       non_null_count: 5
///       mem_size: 40
///       dtype_specific_meta:
///         kind: numeric
///         mean: 78.52000000000001
///         stddev: 20.63545492592785
///         quantiles:
///           min: 45.1
///           25%: 72.3
///           50%: 88.7
///           75%: 91.0
///           max: 95.5
///     - name: active
///       dtype: bool
///       count: 5
///       non_null_count: 5
///       mem_size: 1
///       dtype_specific_meta:
///         kind: categorical
///         n_unique: 2
///         most_common:
///           'true': 3
///           'false': 2
///     - name: birth_date
///       dtype: date
///       count: 5
///       non_null_count: 5
///       mem_size: 20
///       dtype_specific_meta:
///         kind: datetime
///         unit: ms
///         tz: null
///     - name: created_at
///       dtype: datetime[μs]
///       count: 5
///       non_null_count: 5
///       mem_size: 40
///       dtype_specific_meta:
///         kind: datetime
///         unit: us
///         tz: null
///     - name: session_ms
///       dtype: u16
///       count: 5
///       non_null_count: 5
///       mem_size: 10
///       dtype_specific_meta:
///         kind: numeric
///         mean: 13900.0
///         stddev: 17887.70527485289
///         quantiles:
///           min: 300.0
///           25%: 5000.0
///           50%: 7200.0
///           75%: 12000.0
///           max: 45000.0
///     - name: role
///       dtype: str
///       count: 5
///       non_null_count: 5
///       mem_size: 27
///       dtype_specific_meta:
///         kind: categorical
///         n_unique: 3
///         most_common:
///           admin: 2
///           user: 2
///           moderator: 1
///     # END
///
#[derive(clap::Args)]
#[command(verbatim_doc_comment)]
struct MetadataArgs {
    #[command(flatten)]
    shared: SharedOpts,

    #[arg(long, default_value = "yaml")]
    format: String,

    #[arg(num_args = 1..)]
    paths: Vec<std::path::PathBuf>,
}

fn parse_col_spec(raw: &str) -> Result<(String, dataframe_convert::DataTypeSer)> {
    let (col, rest) = raw.split_once('=').context("expected col=type:arg1,arg2")?;
    let col = col.trim().to_string();
    let (dtype_str, args_str) = rest.split_once(':').unwrap_or((rest, ""));
    let args = split_escaped(args_str);
    let ds = parse_dtype(dtype_str, args.as_slice())?;
    Ok((col, ds))
}

fn parse_dtype(raw: &str, args: &[String]) -> Result<dataframe_convert::DataTypeSer> {
    use dataframe_convert::DataTypeSer;
    let raw_lower = raw.to_lowercase();
    let ifmt = find_arg(args, "ifmt");
    let ofmt = find_arg(args, "ofmt");
    let unit = parse_time_unit(args);

    Ok(match raw_lower.as_str() {
        "str" | "string" | "utf8" => DataTypeSer::String {
            strip: args.iter().any(|a| *a == "strip"),
            max_size: find_arg(args, "max_size").and_then(|s| s.parse().ok()),
        },
        "i8" | "int8" => DataTypeSer::Int8,
        "i16" | "int16" => DataTypeSer::Int16,
        "i32" | "int32" => DataTypeSer::Int32,
        "i64" | "int" | "int64" | "integer" => DataTypeSer::Int64,
        "i128" | "int128" => DataTypeSer::Int128,
        "u8" | "uint8" => DataTypeSer::UInt8,
        "u16" | "uint16" => DataTypeSer::UInt16,
        "u32" | "uint32" => DataTypeSer::UInt32,
        "u64" | "uint64" => DataTypeSer::UInt64,
        "u128" | "uint128" => DataTypeSer::UInt128,
        "f16" | "float16" => DataTypeSer::Float16,
        "f32" | "float32" | "float" => DataTypeSer::Float32,
        "f64" | "float64" | "double" => DataTypeSer::Float64,
        "b" | "bool" | "boolean" => DataTypeSer::Bool,
        "date" => DataTypeSer::Date { ifmt, ofmt },
        "time" => DataTypeSer::Time { ifmt, ofmt },
        "duration" | "timedelta" => DataTypeSer::Duration { unit },
        "dt" | "datetime" => DataTypeSer::Datetime {
            ifmt,
            ofmt,
            unit,
            tz: polars::datatypes::TimeZone::opt_try_new(find_arg(args, "tz"))?,
        },
        "cat" | "categorical" => DataTypeSer::Categorical,
        "blob" | "bin" | "binary" | "bytes" => DataTypeSer::Blob,
        _ => bail!("unknown dtype: {raw}"),
    })
}

fn find_arg(args: &[String], key: &str) -> Option<String> {
    for a in args {
        if let Some((k, v)) = a.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn parse_time_unit(args: &[String]) -> polars::prelude::TimeUnit {
    let raw = find_arg(args, "unit").unwrap_or_default();
    match raw.to_lowercase().as_str() {
        "ms" | "milli" | "millis" | "milliseconds" => polars::prelude::TimeUnit::Milliseconds,
        "us" | "micro" | "micros" | "microseconds" => polars::prelude::TimeUnit::Microseconds,
        "ns" | "nano" | "nanos" | "nanoseconds" => polars::prelude::TimeUnit::Nanoseconds,
        _ => polars::prelude::TimeUnit::Microseconds,
    }
}

fn parse_separator(s: &str) -> Result<u8> {
    if s.len() == 1 {
        return Ok(s.as_bytes()[0]);
    }
    bail!("invalid separator: {s:?} (expected a single character)")
}

fn split_escaped(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    let mut backslashes = 0;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if chars.peek() == Some(&'t') {
                    chars.next();
                    current.push('\t');
                    backslashes = 0;
                } else {
                    backslashes += 1;
                    current.push(ch);
                }
            }
            ',' => {
                if backslashes % 2 == 0 {
                    out.push(current);
                    current = String::new();
                } else {
                    current.push(ch);
                }
                backslashes = 0;
            }
            _ => {
                backslashes = 0;
                current.push(ch);
            }
        }
    }

    out.push(current);
    out
}
