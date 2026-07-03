use anyhow::{Context, Result};
use polars::prelude::*;
use polars_excel_writer::PolarsExcelWriter;

pub fn write_xlsx(df: &DataFrame, path: &str, emit_header: bool, _sheet: &str) -> Result<()> {
    let mut w = PolarsExcelWriter::new();
    w.set_header(emit_header);
    w.write_dataframe(df)
        .with_context(|| "failed writing xlsx dataframe")?;
    w.save(path)
        .with_context(|| format!("failed saving xlsx to {path}"))?;
    Ok(())
}

pub fn write_xls(_df: &DataFrame, _path: &str, _emit_header: bool, _sheet: &str) -> Result<()> {
    anyhow::bail!("XLS output is not supported")
}

pub fn write_ods(_df: &DataFrame, _path: &str, _emit_header: bool, _sheet: &str) -> Result<()> {
    anyhow::bail!("ODS output is not supported")
}
