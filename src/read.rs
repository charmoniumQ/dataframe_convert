use anyhow::{Context, Result, bail};
use polars::lazy::frame::IntoLazy;
use polars::prelude::*;
use std::sync::Arc;

use crate::datatype_ser::{DataTypeSer, datatype_ser_to_schema, deserialize_df};
use crate::formats::InputFormat;
use crate::read_excel::{read_ods, read_xls, read_xlsx};

pub fn read_lfs(
    format: InputFormat,
    paths: &[PlRefPath],
    column_datatype_sers: &[(String, DataTypeSer)],
) -> Result<Vec<(String, LazyFrame)>> {
    let mut frames: Vec<(String, LazyFrame)> = Vec::new();
    for path in paths {
        let df = read_lf(format.clone(), path.clone(), column_datatype_sers)
            .with_context(|| format!("failed reading {path}"))?;
        frames.push((format.clone().label(path.clone()), df));
    }
    Ok(frames)
}

pub fn read_lf(
    format: InputFormat,
    path: PlRefPath,
    column_datatype_sers: &[(String, DataTypeSer)],
) -> Result<LazyFrame> {
    let schema = datatype_ser_to_schema(column_datatype_sers, &format);
    let schema_overwrites = if schema.is_empty() {
        None
    } else {
        Some(Arc::new(schema))
    };
    let df = match format.clone() {
        InputFormat::Csv {
            separator,
            has_header,
            ignore_errors,
            skip_rows,
        } => {
            let r = LazyCsvReader::new(path)
                .with_has_header(has_header)
                .with_separator(separator)
                .with_ignore_errors(ignore_errors)
                .with_skip_rows(skip_rows)
                .with_dtype_overwrite(schema_overwrites);
            r.finish()?
        }
        InputFormat::Parquet => LazyFrame::scan_parquet(path, ScanArgsParquet::default())?,
        InputFormat::Json { ignore_errors } => {
            let reader = LazyJsonLineReader::new(path)
                .with_ignore_errors(ignore_errors)
                .with_schema_overwrite(schema_overwrites);
            reader.finish()?
        }
        InputFormat::Ipc => LazyFrame::scan_ipc(
            path,
            polars::io::ipc::IpcScanOptions::default(),
            Default::default(),
        )?,
        InputFormat::Xlsx {
            has_header,
            ignore_errors: _,
        } => read_xlsx(path.as_ref(), has_header, schema_overwrites.clone())?.lazy(),
        InputFormat::Xls {
            has_header,
            ignore_errors: _,
        } => read_xls(path.as_ref(), has_header, schema_overwrites.clone())?.lazy(),
        InputFormat::Ods {
            has_header,
            ignore_errors: _,
        } => read_ods(path.as_ref(), has_header, schema_overwrites.clone())?.lazy(),
        _ => bail!("Unsupported input format {format:?}"),
    };
    let df = deserialize_df(df, column_datatype_sers, &format)?;
    Ok(df)
}

pub fn read_sqlite_lf(path: PlRefPath, table: &str) -> Result<LazyFrame> {
    let conn = rusqlite::Connection::open(&path)
        .with_context(|| format!("failed to open sqlite: {path}"))?;
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM \"{table}\""))
        .with_context(|| format!("failed to prepare query for table '{table}'"))?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap().to_string())
        .collect();
    let mut columns: Vec<Vec<rusqlite::types::Value>> = vec![Vec::new(); col_count];
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        for i in 0..col_count {
            let val: rusqlite::types::Value = row.get(i)?;
            columns[i].push(val);
        }
    }
    let cols: Vec<Column> = col_names
        .into_iter()
        .zip(columns)
        .map(|(name, col)| sqlite_values_to_column(&name, &col))
        .collect();
    Ok(DataFrame::new_infer_height(cols)?.lazy())
}

fn sqlite_values_to_column(name: &str, values: &[rusqlite::types::Value]) -> Column {
    let dtype = values
        .iter()
        .find_map(|v| match v {
            rusqlite::types::Value::Null => None,
            rusqlite::types::Value::Integer(_) => Some(DataType::Int64),
            rusqlite::types::Value::Real(_) => Some(DataType::Float64),
            rusqlite::types::Value::Text(_) => Some(DataType::String),
            rusqlite::types::Value::Blob(_) => Some(DataType::Binary),
        })
        .unwrap_or(DataType::Null);

    match dtype {
        DataType::Int64 => Column::new(
            name.into(),
            values
                .iter()
                .map(|v| match v {
                    rusqlite::types::Value::Integer(i) => *i,
                    _ => 0,
                })
                .collect::<Vec<i64>>(),
        ),
        DataType::Float64 => Column::new(
            name.into(),
            values
                .iter()
                .map(|v| match v {
                    rusqlite::types::Value::Real(f) => *f,
                    rusqlite::types::Value::Integer(i) => *i as f64,
                    _ => 0.0,
                })
                .collect::<Vec<f64>>(),
        ),
        DataType::String => Column::new(
            name.into(),
            values
                .iter()
                .map(|v| match v {
                    rusqlite::types::Value::Text(s) => s.clone(),
                    rusqlite::types::Value::Integer(i) => i.to_string(),
                    rusqlite::types::Value::Real(f) => f.to_string(),
                    rusqlite::types::Value::Blob(b) => format!("{b:?}"),
                    rusqlite::types::Value::Null => String::new(),
                })
                .collect::<Vec<String>>(),
        ),
        DataType::Binary => Column::new(
            name.into(),
            values
                .iter()
                .map(|v| match v {
                    rusqlite::types::Value::Blob(b) => b.clone(),
                    _ => vec![],
                })
                .collect::<Vec<Vec<u8>>>(),
        ),
        _ => Column::new_empty(name.into(), &DataType::Null),
    }
}
