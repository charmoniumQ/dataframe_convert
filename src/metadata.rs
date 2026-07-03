use polars::prelude::*;
use serde::Serialize;

fn serialize_timeunit<S: serde::Serializer>(tu: &TimeUnit, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(match tu {
        TimeUnit::Nanoseconds => "ns",
        TimeUnit::Microseconds => "us",
        TimeUnit::Milliseconds => "ms",
    })
}

fn serialize_tz<S: serde::Serializer>(tz: &Option<TimeZone>, s: S) -> Result<S::Ok, S::Error> {
    match tz {
        None => s.serialize_none(),
        Some(tz) => s.serialize_str(&tz.to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct FrameMetadata {
    pub source: String,
    pub file_size: u64,
    pub mem_size: usize,
    pub overhead: f64,
    pub parse_secs: f64,
    pub n_rows: usize,
    pub columns: Vec<ColumnMetadata>,
}

#[derive(Debug, Serialize)]
pub struct ColumnMetadata {
    pub name: String,
    pub dtype: String,
    pub count: usize,
    pub non_null_count: usize,
    pub mem_size: usize,
    pub dtype_specific_meta: DtypeMetadata,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DtypeMetadata {
    Numeric {
        mean: f64,
        stddev: f64,
        quantiles: Vec<(f64, f64)>,
    },
    Datetime {
        #[serde(skip)]
        min: std::time::Instant,
        #[serde(skip)]
        max: std::time::Instant,
        #[serde(serialize_with = "serialize_timeunit")]
        unit: TimeUnit,
        #[serde(serialize_with = "serialize_tz")]
        tz: Option<TimeZone>,
    },
    Duration {
        min: DurationInfo,
        max: DurationInfo,
        #[serde(serialize_with = "serialize_timeunit")]
        unit: TimeUnit,
    },
    Categorical {
        n_unique: usize,
        n_most_common: Vec<(String, usize)>,
    },
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Serialize, Default)]
pub struct DurationInfo {
    secs: u64,
    nanos: u32,
}

impl From<std::time::Duration> for DurationInfo {
    fn from(d: std::time::Duration) -> Self {
        Self {
            secs: d.as_secs(),
            nanos: d.subsec_nanos(),
        }
    }
}

fn i64_to_duration(val: u64, unit: TimeUnit) -> std::time::Duration {
    match unit {
        TimeUnit::Nanoseconds => std::time::Duration::from_nanos(val),
        TimeUnit::Microseconds => std::time::Duration::from_micros(val),
        TimeUnit::Milliseconds => std::time::Duration::from_millis(val),
    }
}

fn i64_to_instant(val: i64, unit: TimeUnit) -> std::time::Instant {
    let dur = i64_to_duration(val.unsigned_abs(), unit);
    let now = std::time::Instant::now();
    if val >= 0 { now + dur } else { now - dur }
}

fn min_max_i64(series: &Series) -> (Option<i64>, Option<i64>) {
    let ca = series.i64().ok();
    let min = ca.and_then(|c| c.min());
    let max = ca.and_then(|c| c.max());
    (min, max)
}

fn numeric_quantiles(series: &Series) -> Vec<(f64, f64)> {
    let ca = series.f64().ok();
    [0.0, 0.25, 0.5, 0.75, 1.0]
        .iter()
        .filter_map(|&p| {
            let v = ca.and_then(|c| c.quantile(p, QuantileMethod::Linear).ok().flatten());
            Some((p, v?))
        })
        .collect()
}

fn categorical_metadata(series: &Series) -> DtypeMetadata {
    const N_MOST_COMMON: usize = 3;

    let n_unique = series.n_unique().unwrap_or(0);
    let n_most_common = series
        .value_counts(true, true, "count".into(), false)
        .ok()
        .map(|df| {
            let names = df.get_column_names();
            let vals = df.column(names[0].as_str()).unwrap();
            let cnts = df.column(names[1].as_str()).unwrap();
            (0..df.height().min(N_MOST_COMMON))
                .filter_map(|i| {
                    let label = format!("{}", vals.get(i).ok()?);
                    let count = cnts.get(i).ok()?.try_extract::<u32>().unwrap_or(0) as usize;
                    Some((label, count))
                })
                .collect()
        })
        .unwrap_or_default();

    DtypeMetadata::Categorical {
        n_unique,
        n_most_common,
    }
}

fn numeric_metadata(series: &Series) -> DtypeMetadata {
    let s = series.cast(&DataType::Float64).ok();
    let mean = s.as_ref().and_then(|s| s.mean());
    let stddev = s.as_ref().and_then(|s| s.std(1));
    let quantiles = s.as_ref().map(numeric_quantiles).unwrap_or_default();

    DtypeMetadata::Numeric {
        mean: mean.unwrap_or(f64::NAN),
        stddev: stddev.unwrap_or(f64::NAN),
        quantiles,
    }
}

fn datetime_metadata(series: &Series, unit: TimeUnit, tz: Option<TimeZone>) -> DtypeMetadata {
    let (min_val, max_val) = min_max_i64(series);
    let now = std::time::Instant::now();

    DtypeMetadata::Datetime {
        min: min_val.map(|v| i64_to_instant(v, unit)).unwrap_or(now),
        max: max_val.map(|v| i64_to_instant(v, unit)).unwrap_or(now),
        unit,
        tz,
    }
}

fn duration_metadata(series: &Series, unit: TimeUnit) -> DtypeMetadata {
    let (min_val, max_val) = min_max_i64(series);

    DtypeMetadata::Duration {
        min: min_val
            .map(|v| i64_to_duration(v.unsigned_abs(), unit).into())
            .unwrap_or(DurationInfo { secs: 0, nanos: 0 }),
        max: max_val
            .map(|v| i64_to_duration(v.unsigned_abs(), unit).into())
            .unwrap_or(DurationInfo { secs: 0, nanos: 0 }),
        unit,
    }
}

pub fn dtype_to_metadata(series: Series) -> DtypeMetadata {
    match series.dtype() {
        DataType::Boolean => categorical_metadata(&series),
        DataType::Date => {
            if let Ok(s) = series.cast(&DataType::Datetime(TimeUnit::Milliseconds, None)) {
                datetime_metadata(&s, TimeUnit::Milliseconds, None)
            } else {
                DtypeMetadata::None
            }
        }
        DataType::Time => {
            let (min_val, max_val) = min_max_i64(&series);
            let now = std::time::Instant::now();
            DtypeMetadata::Datetime {
                min: min_val
                    .map(|v| i64_to_instant(v, TimeUnit::Nanoseconds))
                    .unwrap_or(now),
                max: max_val
                    .map(|v| i64_to_instant(v, TimeUnit::Nanoseconds))
                    .unwrap_or(now),
                unit: TimeUnit::Nanoseconds,
                tz: None,
            }
        }
        dt if dt.is_numeric() => numeric_metadata(&series),
        DataType::Datetime(unit, tz) => datetime_metadata(&series, *unit, tz.clone()),
        DataType::Duration(unit) => duration_metadata(&series, *unit),
        DataType::String | DataType::Categorical(..) | DataType::Enum(..) => {
            categorical_metadata(&series)
        }
        _ => DtypeMetadata::None,
    }
}

pub fn series_to_metadata(series: &Series) -> ColumnMetadata {
    ColumnMetadata {
        name: series.name().to_string(),
        dtype: series.dtype().to_string(),
        count: series.len(),
        non_null_count: series.len() - series.null_count(),
        mem_size: series.estimated_size(),
        dtype_specific_meta: dtype_to_metadata(series.clone()),
    }
}

pub fn dataframe_to_metadata(
    df: &DataFrame,
    source: &str,
    file_size: u64,
    parse_secs: f64,
) -> anyhow::Result<FrameMetadata> {
    let mem_size = df.estimated_size();
    let columns: Vec<ColumnMetadata> = df
        .columns()
        .iter()
        .map(|col| series_to_metadata(col.as_materialized_series()))
        .collect();
    Ok(FrameMetadata {
        source: source.to_string(),
        file_size,
        mem_size,
        overhead: file_size as f64 / mem_size.max(1) as f64,
        parse_secs,
        n_rows: df.height(),
        columns,
    })
}

pub fn metadata_frame(
    df: DataFrame,
    source: String,
    parse_secs: f64,
    file_size: u64,
) -> FrameMetadata {
    let mem_size = df.estimated_size();
    let columns: Vec<ColumnMetadata> = df
        .columns()
        .iter()
        .map(|col| series_to_metadata(col.as_materialized_series()))
        .collect();
    FrameMetadata {
        source: source.to_string(),
        file_size,
        mem_size,
        overhead: file_size as f64 / mem_size.max(1) as f64,
        parse_secs,
        n_rows: df.height(),
        columns,
    }
}

pub fn lazyframe_to_metadata(
    lf: &mut LazyFrame,
    source: &str,
    file_size: u64,
    parse_secs: f64,
) -> anyhow::Result<FrameMetadata> {
    let df = std::mem::take(lf).collect()?;
    dataframe_to_metadata(&df, source, file_size, parse_secs)
}
