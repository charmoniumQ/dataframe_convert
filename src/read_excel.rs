use anyhow::{Context, Result};
use calamine::{Data, Range, Reader as _};
use polars::prelude::*;
use std::sync::Arc;

fn split_sheet(path: &str) -> (&str, &str) {
    if let Some(pos) = path.rfind(':') {
        (&path[..pos], &path[pos + 1..])
    } else {
        (path, "0")
    }
}

pub fn read_xlsx(path: &str, has_header: bool, schema: Option<Arc<Schema>>) -> Result<DataFrame> {
    let (real_path, sheet) = split_sheet(path);
    let mut wb: calamine::Xlsx<std::io::BufReader<std::fs::File>> =
        calamine::open_workbook(real_path)
            .with_context(|| format!("failed to open xlsx: {path}"))?;
    let range = wb
        .worksheet_range(sheet)
        .map_err(|e| anyhow::anyhow!("sheet not found '{sheet}': {e:?}"))?;
    let df = range_to_string_df(&range, has_header).context("range_to_string")?;
    detect_and_cast(df, schema.as_deref())
}

pub fn read_xls(path: &str, has_header: bool, schema: Option<Arc<Schema>>) -> Result<DataFrame> {
    let (real_path, sheet) = split_sheet(path);
    let mut wb: calamine::Xls<std::io::BufReader<std::fs::File>> =
        calamine::open_workbook(real_path)
            .with_context(|| format!("failed to open xls: {path}"))?;
    let range = wb
        .worksheet_range(sheet)
        .map_err(|e| anyhow::anyhow!("sheet not found '{sheet}': {e:?}"))?;
    let df = range_to_string_df(&range, has_header).context("range_to_string")?;
    detect_and_cast(df, schema.as_deref())
}

pub fn read_ods(path: &str, has_header: bool, schema: Option<Arc<Schema>>) -> Result<DataFrame> {
    let (real_path, sheet) = split_sheet(path);
    let mut wb: calamine::Ods<std::io::BufReader<std::fs::File>> =
        calamine::open_workbook(real_path)
            .with_context(|| format!("failed to open ods: {path}"))?;
    let range = wb
        .worksheet_range(sheet)
        .map_err(|e| anyhow::anyhow!("sheet not found '{sheet}': {e:?}"))?;
    let df = range_to_string_df(&range, has_header).context("range_to_string")?;
    detect_and_cast(df, schema.as_deref())
}

pub fn sheet_names_xlsx(path: &str) -> Result<Vec<String>> {
    let wb: calamine::Xlsx<std::io::BufReader<std::fs::File>> =
        calamine::open_workbook(path).with_context(|| format!("failed to open xlsx: {path}"))?;
    Ok(wb.sheet_names())
}

pub fn sheet_names_xls(path: &str) -> Result<Vec<String>> {
    let wb: calamine::Xls<std::io::BufReader<std::fs::File>> =
        calamine::open_workbook(path).with_context(|| format!("failed to open xls: {path}"))?;
    Ok(wb.sheet_names())
}

pub fn sheet_names_ods(path: &str) -> Result<Vec<String>> {
    let wb: calamine::Ods<std::io::BufReader<std::fs::File>> =
        calamine::open_workbook(path).with_context(|| format!("failed to open ods: {path}"))?;
    Ok(wb.sheet_names())
}

fn range_to_string_df(range: &Range<Data>, has_header: bool) -> Result<DataFrame> {
    let rows: Vec<&[Data]> = range.rows().collect();
    if rows.is_empty() {
        return Ok(DataFrame::empty());
    }

    let n_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    let (header_offset, col_names): (usize, Vec<String>) = if has_header && !rows.is_empty() {
        let names: Vec<String> = (0..n_cols)
            .map(|i| {
                let raw = rows[0].get(i).map(data_to_string).unwrap_or_default();
                if raw.is_empty() {
                    format!("column_{i}")
                } else {
                    raw
                }
            })
            .collect();
        (1, names)
    } else {
        let names: Vec<String> = (0..n_cols).map(|i| format!("column_{i}")).collect();
        (0, names)
    };

    let n_rows = rows.len() - header_offset;

    let mut cols: Vec<Column> = Vec::with_capacity(n_cols);
    for ci in 0..n_cols {
        let mut values: Vec<Option<String>> = Vec::with_capacity(n_rows);
        for ri in header_offset..rows.len() {
            let val = rows[ri].get(ci).and_then(|d| {
                if matches!(d, Data::Empty) {
                    None
                } else {
                    Some(data_to_string(d))
                }
            });
            values.push(val);
        }
        let name = col_names[ci].clone();
        cols.push(Column::new(name.into(), values));
    }

    DataFrame::new(n_rows.max(1), cols).context("Constructing df")
}

fn data_to_string(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => f.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => excel_datetime_to_string(dt.as_f64()),
        Data::Error(e) => format!("{e}"),
        _ => d.to_string(),
    }
}

fn excel_datetime_to_string(serial: f64) -> String {
    let days = serial.trunc() as i64;
    let frac = serial.fract();
    let seconds_in_day = (frac * 86400.0).round() as i64;

    let (y, m, d) = excel_date_to_ymd(days);

    let hours = seconds_in_day / 3600;
    let minutes = (seconds_in_day % 3600) / 60;
    let secs = seconds_in_day % 60;

    if hours == 0 && minutes == 0 && secs == 0 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02} {hours:02}:{minutes:02}:{secs:02}")
    }
}

fn excel_date_to_ymd(mut days: i64) -> (i64, u32, u32) {
    days += 2;

    let mut year = 1900i64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }

    (year, month, (days + 1) as u32)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[derive(Clone, Debug)]
enum DetectedType {
    Integer,
    Float,
    Bool,
    Date(String),
    Datetime(String),
    String,
}

fn schema_to_detected(dtype: &DataType) -> DetectedType {
    match dtype {
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::Int128
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::UInt128 => DetectedType::Integer,
        DataType::Float16 | DataType::Float32 | DataType::Float64 => DetectedType::Float,
        DataType::Boolean => DetectedType::Bool,
        DataType::Date => DetectedType::Date("%Y-%m-%d".into()),
        DataType::Datetime(_, _) => DetectedType::Datetime("%Y-%m-%d %H:%M:%S".into()),
        _ => DetectedType::String,
    }
}

fn detect_and_cast(df: DataFrame, schema: Option<&Schema>) -> Result<DataFrame> {
    let col_names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|n| n.to_string())
        .collect();

    let detections: Vec<DetectedType> = col_names
        .iter()
        .map(|n| {
            schema
                .and_then(|s| s.get(n.as_str()))
                .map(schema_to_detected)
                .unwrap_or(DetectedType::String)
        })
        .collect();

    let mut lf = df.lazy();

    for (col_name, dtype) in col_names.iter().zip(detections.iter()) {
        match dtype {
            DetectedType::Integer => {
                lf = lf.with_column(
                    col(col_name.as_str())
                        .cast(DataType::Int64)
                        .alias(col_name.as_str()),
                );
            }
            DetectedType::Float => {
                lf = lf.with_column(
                    col(col_name.as_str())
                        .cast(DataType::Float64)
                        .alias(col_name.as_str()),
                );
            }
            DetectedType::Bool => {
                lf = lf.with_column(
                    col(col_name.as_str())
                        .cast(DataType::Boolean)
                        .alias(col_name.as_str()),
                );
            }
            DetectedType::Date(fmt) => {
                lf = lf.with_column(
                    col(col_name.as_str())
                        .str()
                        .strptime(
                            DataType::Date,
                            StrptimeOptions {
                                format: Some(fmt.clone().into()),
                                strict: false,
                                exact: false,
                                cache: false,
                            },
                            lit("raise"),
                        )
                        .alias(col_name.as_str()),
                );
            }
            DetectedType::Datetime(fmt) => {
                lf = lf.with_column(
                    col(col_name.as_str())
                        .str()
                        .strptime(
                            DataType::Datetime(TimeUnit::Microseconds, None),
                            StrptimeOptions {
                                format: Some(fmt.clone().into()),
                                strict: false,
                                exact: false,
                                cache: false,
                            },
                            lit("raise"),
                        )
                        .alias(col_name.as_str()),
                );
            }
            DetectedType::String => {}
        }
    }

    Ok(lf.collect()?)
}
