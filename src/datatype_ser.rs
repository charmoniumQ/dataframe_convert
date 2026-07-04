use crate::formats::{InputFormat, OutputFormat};
use anyhow::Result;
use polars::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum DataTypeSer {
    String {
        strip: bool,
        max_size: Option<usize>,
    },
    Int8,
    Int16,
    Int32,
    Int64,
    Int128,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    UInt128,
    Float16,
    Float32,
    Float64,
    Bool,
    Date {
        ifmt: Option<String>,
        ofmt: Option<String>,
    },
    Time {
        ifmt: Option<String>,
        ofmt: Option<String>,
    },
    Datetime {
        ifmt: Option<String>,
        ofmt: Option<String>,
        unit: TimeUnit,
        tz: Option<TimeZone>,
    },
    Duration {
        unit: TimeUnit,
    },
    Categorical,
    Enum {
        items: Vec<String>,
    },
    Blob,
    Uuid,
}

impl DataTypeSer {
    pub fn get_input_datatype(&self, infmt: &InputFormat) -> DataType {
        match self {
            Self::String { .. } => DataType::String,
            Self::Int8 => DataType::Int8,
            Self::Int16 => DataType::Int16,
            Self::Int32 => DataType::Int32,
            Self::Int64 => {
                if matches!(infmt, InputFormat::Json { .. }) {
                    DataType::String
                } else {
                    DataType::Int64
                }
            }
            Self::UInt64 => {
                if matches!(infmt, InputFormat::Json { .. }) {
                    DataType::String
                } else {
                    DataType::UInt64
                }
            }
            Self::UInt128 => {
                if matches!(infmt, InputFormat::Json { .. }) {
                    DataType::String
                } else {
                    DataType::UInt128
                }
            }
            Self::Int128 => DataType::Int128,
            Self::UInt8 => DataType::UInt8,
            Self::UInt16 => DataType::UInt16,
            Self::UInt32 => DataType::UInt32,
            Self::Float16 => DataType::Float16,
            Self::Float32 => DataType::Float32,
            Self::Float64 => DataType::Float64,
            Self::Bool => DataType::Boolean,
            Self::Date { ifmt, .. } => {
                if ifmt.is_some() {
                    DataType::String
                } else {
                    DataType::Date
                }
            }
            Self::Time { ifmt, .. } => {
                if ifmt.is_some() {
                    DataType::String
                } else {
                    DataType::Time
                }
            }
            Self::Datetime { ifmt, unit, .. } => {
                if ifmt.is_some() {
                    DataType::String
                } else {
                    DataType::Datetime(*unit, None)
                }
            }
            Self::Duration { unit } => {
                if format_supports_duration_in(infmt) {
                    DataType::Duration(*unit)
                } else {
                    DataType::Int64
                }
            }
            Self::Categorical => DataType::Categorical(
                Categories::random("cats".into(), polars::prelude::CategoricalPhysical::U32),
                CategoricalMapping::with_hasher(
                    0xFFFFFFFF,
                    foldhash::quality::SeedableRandomState::fixed(),
                )
                .into(),
            ),
            Self::Enum { items } => DataType::Enum(
                FrozenCategories::new(items.iter().map(|s| s.as_str())).unwrap(),
                CategoricalMapping::with_hasher(
                    0xFFFFFFFF,
                    foldhash::quality::SeedableRandomState::fixed(),
                )
                .into(),
            ),
            Self::Blob => DataType::Binary,
            Self::Uuid => DataType::String,
        }
    }

    pub fn deserialize_column(&self, mut col: Expr, infmt: &InputFormat) -> Expr {
        match self {
            Self::String { strip, max_size } => {
                if *strip {
                    col = col.str().strip_chars(lit(""));
                }
                if let Some(max_size) = max_size {
                    col = col.str().slice(lit(NULL), lit(*max_size as u32));
                }
            }
            Self::Date {
                ifmt: Some(ifmt), ..
            } => {
                col = col.str().strptime(
                    DataType::Date,
                    StrptimeOptions {
                        format: Some(ifmt.into()),
                        strict: false,
                        exact: false,
                        cache: false,
                    },
                    lit("raise"),
                );
            }
            Self::Time {
                ifmt: Some(ifmt), ..
            } => {
                col = col.str().strptime(
                    DataType::Time,
                    StrptimeOptions {
                        format: Some(ifmt.into()),
                        strict: false,
                        exact: false,
                        cache: false,
                    },
                    lit("raise"),
                );
            }
            Self::Duration { unit } => {
                if !format_supports_duration_in(infmt) {
                    col = col.cast(DataType::Duration(*unit));
                }
            }
            Self::Datetime {
                ifmt: Some(ifmt),
                unit,
                tz,
                ..
            } => {
                col = col.str().strptime(
                    DataType::Datetime(*unit, tz.clone()),
                    StrptimeOptions {
                        format: Some(ifmt.into()),
                        strict: false,
                        exact: false,
                        cache: false,
                    },
                    lit("raise"),
                );
            }
            Self::Int64 if matches!(infmt, InputFormat::Json { .. }) => {
                col = col.cast(DataType::String);
            }
            Self::UInt64 if matches!(infmt, InputFormat::Json { .. }) => {
                col = col.cast(DataType::String);
            }
            Self::UInt128 if matches!(infmt, InputFormat::Json { .. }) => {
                col = col.cast(DataType::String);
            }
            _ => {}
        }
        col
    }

    pub fn serialize_column(&self, mut col: Expr, outf: &OutputFormat) -> Expr {
        match self {
            Self::Date {
                ofmt: Some(ofmt), ..
            } => {
                col = col.dt().strftime(ofmt.as_str());
            }
            Self::Time {
                ofmt: Some(ofmt), ..
            } => {
                col = col.dt().strftime(ofmt.as_str());
            }
            Self::Duration { unit: _ } => {
                if !format_supports_duration_out(outf) {
                    col = col.cast(DataType::Int64);
                }
            }
            Self::Datetime {
                ofmt: Some(ofmt), ..
            } => {
                col = col.dt().strftime(ofmt.as_str());
            }
            Self::Int64 | Self::UInt64 | Self::UInt128 => {
                if matches!(outf, OutputFormat::Json) {
                    col = col.cast(DataType::String);
                }
            }
            Self::Uuid => unimplemented!(),
            _ => {}
        }
        col
    }
}

fn format_supports_duration_in(f: &InputFormat) -> bool {
    matches!(f, InputFormat::Parquet | InputFormat::Ipc)
}

fn format_supports_duration_out(f: &OutputFormat) -> bool {
    matches!(f, OutputFormat::Parquet | OutputFormat::Ipc)
}

fn format_supports_nesting(f: &OutputFormat) -> bool {
    matches!(f, OutputFormat::Parquet | OutputFormat::Ipc | OutputFormat::Json)
}

pub fn datatype_ser_to_schema(
    column_datatype_sers: &[(String, DataTypeSer)],
    infmt: &InputFormat,
) -> Schema {
    let mut s = Schema::default();
    for (col, ds) in column_datatype_sers {
        s.with_column(col.clone().into(), ds.get_input_datatype(infmt));
    }
    s
}

pub fn deserialize_df(
    mut df: LazyFrame,
    column_datatype_sers: &[(String, DataTypeSer)],
    infmt: &InputFormat,
) -> Result<LazyFrame> {
    for (col_name, ds) in column_datatype_sers {
        df = df.with_column(
            ds.deserialize_column(col(col_name.as_str()), infmt)
                .alias(col_name.as_str()),
        );
    }
    Ok(df)
}

pub fn serialize_df(
    mut df: LazyFrame,
    column_datatype_sers: &[(String, DataTypeSer)],
    outf: &OutputFormat,
) -> Result<LazyFrame> {
    for (col_name, ds) in column_datatype_sers {
        df = df.with_column(
            ds.serialize_column(col(col_name.as_str()), outf)
                .alias(col_name.as_str()),
        );
    }

    if !format_supports_nesting(outf) {
        let schema = df.collect_schema()?;
        let specified: std::collections::BTreeSet<&str> = column_datatype_sers
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        for (name, dtype) in schema.iter() {
            if specified.contains(name.as_str()) {
                continue;
            }
            if matches!(dtype, DataType::List(_)) {
                df = df.with_column(
                    col(name.as_str())
                        .list()
                        .eval(col(PlSmallStr::EMPTY).cast(DataType::String))
                        .list()
                        .join(lit(","), true)
                        .alias(name.as_str()),
                );
            } else if matches!(dtype, DataType::Struct(_)) {
                df = df.with_column(col(name.as_str()).cast(DataType::String).alias(name.as_str()));
            }
        }
    }

    Ok(df)
}
