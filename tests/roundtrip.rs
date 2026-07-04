use dataframe_convert::*;
use polars::prelude::*;
use rstest::rstest;
use tempfile::TempDir;

fn dtypes() -> Vec<(String, DataTypeSer)> {
    vec![
        ("ints".into(), DataTypeSer::Int64),
        ("floats".into(), DataTypeSer::Float64),
        (
            "strs".into(),
            DataTypeSer::String {
                strip: false,
                max_size: None,
            },
        ),
        ("bools".into(), DataTypeSer::Bool),
        (
            "dates".into(),
            DataTypeSer::Date {
                ifmt: Some("%Y-%m-%d".into()),
                ofmt: Some("%Y-%m-%d".into()),
            },
        ),
        (
            "datetimes".into(),
            DataTypeSer::Datetime {
                ifmt: Some("%Y-%m-%dT%H:%M:%S".into()),
                ofmt: Some("%Y-%m-%dT%H:%M:%S".into()),
                unit: TimeUnit::Microseconds,
                tz: None,
            },
        ),
        (
            "durations".into(),
            DataTypeSer::Duration {
                unit: TimeUnit::Microseconds,
            },
        ),
        ("cats".into(), DataTypeSer::Categorical),
    ]
}

fn make_df() -> DataFrame {
    df!(
        "ints"      => &[1i64, 2, 3, -5, 0],
        "floats"    => &[1.5f64, -2.0, 3.14, 0.0, 100.0],
        "strs"      => &["hello", "world", "", "a b", "x"],
        "bools"     => &[true, false, true, true, false],
        "dates_str" => &["2024-01-15", "2023-12-31", "2020-06-01", "1999-03-14", "2025-12-25"],
        "dts_str"   => &[
            "2024-01-15T10:30:00",
            "2023-12-31T23:59:59",
            "2020-06-01T00:00:00",
            "1999-03-14T15:30:00",
            "2025-12-25T12:00:00",
        ],
        "cats"      => &["alpha", "beta", "alpha", "gamma", "beta"],
    )
    .unwrap()
    .lazy()
    .with_column(
        col("dates_str")
            .str()
            .strptime(
                DataType::Date,
                StrptimeOptions {
                    format: Some("%Y-%m-%d".into()),
                    strict: false,
                    exact: false,
                    cache: false,
                },
                lit("raise"),
            )
            .alias("dates"),
    )
    .with_column(
        col("dts_str")
            .str()
            .strptime(
                DataType::Datetime(TimeUnit::Microseconds, None),
                StrptimeOptions {
                    format: Some("%Y-%m-%dT%H:%M:%S".into()),
                    strict: false,
                    exact: false,
                    cache: false,
                },
                lit("raise"),
            )
            .alias("datetimes"),
    )
    .with_column(col("cats").cast(DataTypeSer::Categorical.get_input_datatype(&InputFormat::Csv { separator: b',', has_header: true, ignore_errors: false, skip_rows: 0 })))
    .with_column(
        col("ints")
            .cast(DataType::Duration(TimeUnit::Microseconds))
            .alias("durations"),
    )
    .select([
        col("ints"),
        col("floats"),
        col("strs"),
        col("bools"),
        col("dates"),
        col("datetimes"),
        col("durations"),
        col("cats"),
    ])
    .collect()
    .unwrap()
}

#[rstest]
#[case::csv(
    OutputFormat::Csv { separator: b',', emit_header: true },
    InputFormat::Csv  { separator: b',', has_header: true, ignore_errors: false, skip_rows: 0 }
)]
#[case::csv_tsv(
    OutputFormat::Csv { separator: b'\t', emit_header: true },
    InputFormat::Csv  { separator: b'\t', has_header: true, ignore_errors: false, skip_rows: 0 }
)]
#[case::parquet(OutputFormat::Parquet, InputFormat::Parquet)]
#[case::json(
    OutputFormat::Json,
    InputFormat::Json { ignore_errors: false }
)]
#[case::ipc(OutputFormat::Ipc, InputFormat::Ipc)]
fn roundtrip_formats(#[case] out_fmt: OutputFormat, #[case] in_fmt: InputFormat) {
    let original = make_df();
    let dtypes = dtypes();

    let dir = TempDir::new().unwrap();
    let tmp = dir.path().join("test");
    let pl_path: PlRefPath = tmp.to_str().unwrap().into();

    write_lf(
        original.clone().lazy(),
        out_fmt.clone(),
        pl_path.clone(),
        &dtypes,
    )
    .unwrap_or_else(|e| panic!("{out_fmt:?}: write failed: {e}"));

    let rt = read_lf(in_fmt.clone(), pl_path, &dtypes)
        .unwrap_or_else(|e| panic!("{in_fmt:?}: read failed: {e}"))
        .collect()
        .unwrap_or_else(|e| panic!("{in_fmt:?}: collect failed: {e}"));

    assert_eq!(original.height(), rt.height(), "row count mismatch");
    assert_eq!(original.width(), rt.width(), "column count mismatch");

    let orig_meta = metadata_frame(original.clone(), "original".to_string(), 0.0, 0);
    let rt_meta = metadata_frame(rt.clone(), "roundtrip".to_string(), 0.0, 0);

    assert_eq!(orig_meta.n_rows, rt_meta.n_rows, "meta row count mismatch");
    assert_eq!(
        orig_meta.columns.len(),
        rt_meta.columns.len(),
        "meta column count mismatch"
    );

    for (a, b) in orig_meta.columns.iter().zip(rt_meta.columns.iter()) {
        assert_eq!(a.name, b.name, "column name mismatch");
        assert_eq!(a.count, b.count, "column count mismatch: {}", a.name);
        assert_eq!(
            a.non_null_count, b.non_null_count,
            "column non-null mismatch: {}",
            a.name
        );

        match (&a.dtype_specific_meta, &b.dtype_specific_meta) {
            (
                DtypeMetadata::Numeric {
                    mean: a_mean,
                    stddev: a_std,
                    quantiles: a_iles,
                },
                DtypeMetadata::Numeric {
                    mean: b_mean,
                    stddev: b_std,
                    quantiles: b_iles,
                },
            ) => {
                let eps = 1e-4;
                assert!(
                    (a_mean - b_mean).abs() < eps,
                    "mean mismatch: {} vs {} for {}",
                    a_mean,
                    b_mean,
                    a.name
                );
                assert!(
                    (a_std - b_std).abs() < eps || a_std.is_nan() && b_std.is_nan(),
                    "stddev mismatch: {} vs {} for {}",
                    a_std,
                    b_std,
                    a.name
                );
            }
            (
                DtypeMetadata::Duration {
                    min: a_min,
                    max: a_max,
                    ..
                },
                DtypeMetadata::Duration {
                    min: b_min,
                    max: b_max,
                    ..
                },
            ) => {
                assert_eq!(
                    a_min.secs, b_min.secs,
                    "duration min secs mismatch: {}",
                    a.name
                );
                assert_eq!(
                    a_min.nanos, b_min.nanos,
                    "duration min nanos mismatch: {}",
                    a.name
                );
                assert_eq!(
                    a_max.secs, b_max.secs,
                    "duration max secs mismatch: {}",
                    a.name
                );
                assert_eq!(
                    a_max.nanos, b_max.nanos,
                    "duration max nanos mismatch: {}",
                    a.name
                );
            }
            (
                DtypeMetadata::Categorical { n_unique: a_nu, .. },
                DtypeMetadata::Categorical { n_unique: b_nu, .. },
            ) => {
                assert_eq!(a_nu, b_nu, "n_unique mismatch: {}", a.name);
            }
            _ => {}
        }
    }
}

#[test]
fn xlsx_roundtrip() {
    let df = df!(
        "ID"   => &[1i64, 2, 3],
        "Name" => &["alice", "bob", "carol"],
    )
    .unwrap();

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.xlsx");
    let path_str = path.to_str().unwrap();

    dataframe_convert::write_excel::write_xlsx(&df, path_str, true, "0").unwrap();

    let sheets = dataframe_convert::read_excel::sheet_names_xlsx(path_str).unwrap();
    let rt =
        dataframe_convert::read_excel::read_xlsx(&format!("{path_str}:{}", sheets[0]), true, None)
            .unwrap();

    assert_eq!(df.height(), rt.height(), "row count mismatch");
    assert_eq!(df.width(), rt.width(), "column count mismatch");
    assert_eq!(df.get_column_names(), rt.get_column_names());
}

#[test]
fn read_csv() {
    use std::io::Write;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.csv");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "a,b,c").unwrap();
    writeln!(f, "1,hello,3.5").unwrap();
    writeln!(f, "2,world,7.2").unwrap();
    drop(f);

    let df = read_lf(
        InputFormat::Csv {
            separator: b',',
            has_header: true,
            ignore_errors: false,
            skip_rows: 0,
        },
        path.to_str().unwrap().into(),
        &[],
    )
    .unwrap()
    .collect()
    .unwrap();

    assert_eq!(df.height(), 2);
    assert_eq!(df.width(), 3);
    assert_eq!(
        df.get_column_names()
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[test]
fn list_csv_roundtrip_as_string() {
    let numbers = Series::new(
        PlSmallStr::from("numbers"),
        &[
            AnyValue::List(Series::new(PlSmallStr::EMPTY, &[1i64, 2])),
            AnyValue::List(Series::new(PlSmallStr::EMPTY, &[3i64, 4, 5])),
        ],
    );
    let names = Series::new(PlSmallStr::from("name"), &["a", "b"]);
    let df = DataFrame::new(2, vec![numbers.into(), names.into()]).unwrap();

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.csv");
    let path_str = path.to_str().unwrap();

    write_lf(
        df.clone().lazy(),
        OutputFormat::Csv {
            separator: b',',
            emit_header: true,
        },
        path_str.into(),
        &[],
    )
    .unwrap();

    let rt = read_lf(
        InputFormat::Csv {
            separator: b',',
            has_header: true,
            ignore_errors: false,
            skip_rows: 0,
        },
        path_str.into(),
        &[],
    )
    .unwrap()
    .collect()
    .unwrap();

    assert_eq!(rt.column("numbers").unwrap().dtype(), &DataType::String);
    assert_eq!(rt.column("name").unwrap().dtype(), &DataType::String);
    assert_eq!(rt.height(), 2);
}
