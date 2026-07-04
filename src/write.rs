use anyhow::{Context, Result, bail};
use polars::prelude::*;
use polars_excel_writer::PolarsExcelWriter;

use crate::datatype_ser::{DataTypeSer, serialize_df};
use crate::formats::OutputFormat;

pub fn write_lf(
    df: LazyFrame,
    format: OutputFormat,
    path: PlRefPath,
    column_datatype_sers: &[(String, DataTypeSer)],
) -> Result<()> {
    let lf = serialize_df(df, column_datatype_sers, &format)?;
    match format {
        OutputFormat::Csv {
            separator,
            emit_header,
        } => {
            let mut opts = CsvWriterOptions::default();
            Arc::make_mut(&mut opts.serialize_options).separator = separator;
            opts.include_header = emit_header;
            lf.sink(
                SinkDestination::File {
                    target: SinkTarget::Path(path),
                },
                FileWriteFormat::Csv(opts),
                UnifiedSinkArgs::default(),
            )?
            .collect()?;
        }
        OutputFormat::Parquet => {
            lf.sink(
                SinkDestination::File {
                    target: SinkTarget::Path(path),
                },
                FileWriteFormat::Parquet(Arc::new(ParquetWriteOptions::default())),
                UnifiedSinkArgs::default(),
            )?
            .collect()?;
        }
        OutputFormat::Json => {
            lf.sink(
                SinkDestination::File {
                    target: SinkTarget::Path(path),
                },
                FileWriteFormat::NDJson(NDJsonWriterOptions::default()),
                UnifiedSinkArgs::default(),
            )?
            .collect()?;
        }
        OutputFormat::Ipc => {
            lf.sink(
                SinkDestination::File {
                    target: SinkTarget::Path(path),
                },
                FileWriteFormat::Ipc(IpcWriterOptions::default()),
                UnifiedSinkArgs::default(),
            )?
            .collect()?;
        }
        OutputFormat::Xlsx { emit_header } => {
            let df = lf.collect()?;
            let mut w = PolarsExcelWriter::new();
            w.set_header(emit_header);
            w.write_dataframe(&df)
                .with_context(|| "failed writing xlsx dataframe")?;
            w.save(&path)
                .with_context(|| format!("failed saving xlsx to {path}"))?;
        }
        _ => bail!("Unsupported output format {format:?}"),
    };
    Ok(())
}
