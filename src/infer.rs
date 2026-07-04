use polars::prelude::*;
use regex::Regex;
use std::sync::LazyLock;

const SAMPLE_SIZE: usize = 1000;
const STRING_RATIO: f64 = 0.999;
const DURATION_RATIO: f64 = 0.9;
const CATEGORY_RATIO: usize = 5;
const REASONABLE_TIME_WINDOW: std::time::Duration = std::time::Duration::from_secs(315_576_000); // 10 years

static RE_UINT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static RE_INT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+$").unwrap());
static RE_FLOAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$").unwrap());

static RE_ISO_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());
static RE_ISO_DATETIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?$").unwrap());
static RE_ISO_TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{2}:\d{2}:\d{2}$").unwrap());
static RE_EU_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{1,2}/\d{1,2}/\d{4}$").unwrap());
static RE_US_DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{1,2}/\d{1,2}/\d{4}$").unwrap());
static RE_DT_LOCALE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{1,2}/\d{1,2}/\d{4}, \d{1,2}:\d{2}:\d{2} [AP]M \S+$").unwrap()
});

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DateLocale {
    European,
    American,
    Auto,
}

pub fn infer_df(lf: LazyFrame, schema: &Schema, date_locale: DateLocale) -> LazyFrame {
    let Ok(sample) = lf.clone().limit(SAMPLE_SIZE as u32).collect() else {
        return lf;
    };

    let mut lf = lf;

    for name in all_column_names(&sample) {
        if schema.get(name.as_str()).is_some() {
            continue;
        }
        lf = infer_column(lf, &name, &sample, date_locale);
    }

    lf
}

pub fn infer_column(
    lf: LazyFrame,
    name: &PlSmallStr,
    sample: &DataFrame,
    date_locale: DateLocale,
) -> LazyFrame {
    let s = match sample.column(name.as_str()) {
        Ok(s) => s.as_materialized_series(),
        Err(_) => return lf,
    };

    let (lf, became_int) = if s.dtype() == &DataType::String {
        infer_string_column(lf, name, s, date_locale)
    } else {
        (lf, false)
    };

    if became_int || s.dtype().is_integer() {
        return infer_integer_column(lf, name, s);
    }

    lf
}

fn infer_string_column(
    mut lf: LazyFrame,
    name: &PlSmallStr,
    series: &Series,
    date_locale: DateLocale,
) -> (LazyFrame, bool) {
    let ca = series.str().unwrap();

    if let (Some(expr), true) = string_to_uint(ca, name) {
        lf = lf.with_column(expr);
        return (lf, true);
    }
    if let (Some(expr), true) = string_to_int(ca, name) {
        lf = lf.with_column(expr);
        return (lf, true);
    }
    if let Some(expr) = string_to_float(ca, name) {
        lf = lf.with_column(expr);
        return (lf, false);
    }
    if let Some(expr) = string_to_dt(ca, name, date_locale) {
        lf = lf.with_column(expr);
        return (lf, false);
    }
    if let Some(expr) = string_to_date(ca, name, date_locale) {
        lf = lf.with_column(expr);
        return (lf, false);
    }
    if let Some(expr) = string_to_time(ca, name) {
        lf = lf.with_column(expr);
        return (lf, false);
    }
    if let Some(expr) = string_to_bool(ca, name) {
        lf = lf.with_column(expr);
        return (lf, false);
    }
    if let Some(expr) = string_to_categorical(ca, name) {
        lf = lf.with_column(expr);
        return (lf, false);
    }

    (lf, false)
}

fn infer_integer_column(
    mut lf: LazyFrame,
    name: &PlSmallStr,
    series: &Series,
) -> LazyFrame {
    let Ok(s64) = series.cast(&DataType::Int64) else {
        return lf;
    };
    let ca = s64.i64().unwrap();

    if let Some(expr) = int_to_bool(ca, name) {
        lf = lf.with_column(expr);
        return lf;
    }

    if let Some(expr) = int_to_datetime(ca, name) {
        lf = lf.with_column(expr);
        return lf;
    }

    if let Some(expr) = narrow_int(series, name) {
        lf = lf.with_column(expr);
    }

    lf
}

fn string_to_uint(ca: &StringChunked, name: &PlSmallStr) -> (Option<Expr>, bool) {
    let (total, matches) = count_regex(ca, &RE_UINT);
    if total > 0 && matches as f64 / total as f64 > STRING_RATIO {
        return (
            Some(
                col(name.as_str())
                    .cast(DataType::UInt128)
                    .alias(name.as_str()),
            ),
            true,
        );
    }
    (None, false)
}

fn string_to_int(ca: &StringChunked, name: &PlSmallStr) -> (Option<Expr>, bool) {
    let (total, matches) = count_regex(ca, &RE_INT);
    if total > 0 && matches as f64 / total as f64 > STRING_RATIO {
        return (
            Some(
                col(name.as_str())
                    .cast(DataType::Int64)
                    .alias(name.as_str()),
            ),
            true,
        );
    }
    (None, false)
}

fn string_to_float(ca: &StringChunked, name: &PlSmallStr) -> Option<Expr> {
    let (_total, int_matches) = count_regex(ca, &RE_INT);
    let (total, float_matches) = count_regex(ca, &RE_FLOAT);
    let float_only = float_matches.saturating_sub(int_matches);
    if total > 0 {
        let ratio = (int_matches + float_only) as f64 / total as f64;
        if ratio > STRING_RATIO {
            return Some(
                col(name.as_str())
                    .cast(DataType::Float32)
                    .alias(name.as_str()),
            );
        }
    }
    None
}

fn string_to_dt(
    ca: &StringChunked,
    name: &PlSmallStr,
    date_locale: DateLocale,
) -> Option<Expr> {
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if regex_ratio(ca, &RE_ISO_DATETIME) > STRING_RATIO {
            return Some(
                col(name.as_str())
                    .str()
                    .strptime(
                        DataType::Datetime(TimeUnit::Microseconds, None),
                        StrptimeOptions {
                            format: Some(fmt.into()),
                            strict: false,
                            exact: false,
                            cache: false,
                        },
                        lit("raise"),
                    )
                    .alias(name.as_str()),
            );
        }
    }

    if regex_ratio(ca, &RE_DT_LOCALE) > STRING_RATIO {
        let fmt = match date_locale {
            DateLocale::European => "%d/%m/%Y, %I:%M:%S %p %Z",
            DateLocale::American => "%m/%d/%Y, %I:%M:%S %p %Z",
            DateLocale::Auto => {
                if is_european_locale() {
                    "%d/%m/%Y, %I:%M:%S %p %Z"
                } else {
                    "%m/%d/%Y, %I:%M:%S %p %Z"
                }
            }
        };
        return Some(
            col(name.as_str())
                .str()
                .strptime(
                    DataType::Datetime(TimeUnit::Microseconds, None),
                    StrptimeOptions {
                        format: Some(fmt.into()),
                        strict: false,
                        exact: false,
                        cache: false,
                    },
                    lit("raise"),
                )
                .alias(name.as_str()),
        );
    }

    None
}

fn string_to_date(
    ca: &StringChunked,
    name: &PlSmallStr,
    date_locale: DateLocale,
) -> Option<Expr> {
    if regex_ratio(ca, &RE_ISO_DATE) > STRING_RATIO {
        return Some(
            col(name.as_str())
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
                .alias(name.as_str()),
        );
    }

    let (re, fmt) = match date_locale {
        DateLocale::European => (&RE_EU_DATE, "%d/%m/%Y"),
        DateLocale::American => (&RE_US_DATE, "%m/%d/%Y"),
        DateLocale::Auto => {
            if is_european_locale() {
                (&RE_EU_DATE, "%d/%m/%Y")
            } else {
                (&RE_US_DATE, "%m/%d/%Y")
            }
        }
    };

    if regex_ratio(ca, re) > STRING_RATIO {
        return Some(
            col(name.as_str())
                .str()
                .strptime(
                    DataType::Date,
                    StrptimeOptions {
                        format: Some(fmt.into()),
                        strict: false,
                        exact: false,
                        cache: false,
                    },
                    lit("raise"),
                )
                .alias(name.as_str()),
        );
    }

    None
}

fn string_to_time(ca: &StringChunked, name: &PlSmallStr) -> Option<Expr> {
    if regex_ratio(ca, &RE_ISO_TIME) > STRING_RATIO {
        return Some(
            col(name.as_str())
                .str()
                .strptime(
                    DataType::Time,
                    StrptimeOptions {
                        format: Some("%H:%M:%S".into()),
                        strict: false,
                        exact: false,
                        cache: false,
                    },
                    lit("raise"),
                )
                .alias(name.as_str()),
        );
    }
    None
}

fn string_to_bool(ca: &StringChunked, name: &PlSmallStr) -> Option<Expr> {
    let n_unique = ca.n_unique().unwrap_or(0);
    let null_count = ca.null_count();

    if n_unique == 1 {
        if null_count > 0 {
            return Some(
                when(col(name.as_str()).is_not_null())
                    .then(lit(true))
                    .otherwise(lit(false))
                    .alias(name.as_str()),
            );
        }
        return Some(lit(true).alias(name.as_str()));
    }

    if n_unique == 2 {
        let mut cats: Vec<String> = Vec::new();
        for v in ca.into_iter().flatten() {
            let s = v.to_string();
            if !cats.contains(&s) {
                cats.push(s);
            }
            if cats.len() > 2 {
                break;
            }
        }
        if cats.len() == 2 {
            let (t, f) = bool_pair(&cats[0], &cats[1]);
            if let (Some(true_val), Some(_)) = (t, f) {
                return Some(
                    when(col(name.as_str()).is_null())
                        .then(lit(NULL))
                        .when(col(name.as_str()).eq(lit(true_val.as_str())))
                        .then(lit(true))
                        .otherwise(lit(false))
                        .alias(name.as_str()),
                );
            }
        }
    }

    None
}

fn string_to_categorical(ca: &StringChunked, name: &PlSmallStr) -> Option<Expr> {
    let n_unique = ca.n_unique().unwrap_or(0);
    let n_total = ca.len();
    if n_total > 0 && n_unique > 0 && n_total / n_unique > CATEGORY_RATIO {
        return Some(
            col(name.as_str())
                .cast(DataType::Categorical(
                    Categories::random("cats".into(), CategoricalPhysical::U32),
                    CategoricalMapping::with_hasher(
                        0xFFFFFFFF,
                        foldhash::quality::SeedableRandomState::fixed(),
                    )
                    .into(),
                ))
                .alias(name.as_str()),
        );
    }
    None
}

fn int_to_bool(ca: &polars::prelude::Int64Chunked, name: &PlSmallStr) -> Option<Expr> {
    if !ca.into_iter().flatten().all(|v| v == 0 || v == 1) {
        return None;
    }
    Some(
        when(col(name.as_str()).is_null())
            .then(lit(NULL))
            .when(col(name.as_str()).eq(lit(1i64)))
            .then(lit(true))
            .otherwise(lit(false))
            .alias(name.as_str()),
    )
}

fn int_to_datetime(ca: &polars::prelude::Int64Chunked, name: &PlSmallStr) -> Option<Expr> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let candidates: &[(TimeUnit, i64, i64, i64)] = &[
        (
            TimeUnit::Microseconds,
            now.as_micros().saturating_sub(REASONABLE_TIME_WINDOW.as_micros()) as i64,
            now.as_micros().saturating_add(REASONABLE_TIME_WINDOW.as_micros()) as i64,
            1,
        ),
        (
            TimeUnit::Milliseconds,
            now.as_millis().saturating_sub(REASONABLE_TIME_WINDOW.as_millis()) as i64,
            now.as_millis().saturating_add(REASONABLE_TIME_WINDOW.as_millis()) as i64,
            1,
        ),
        (
            TimeUnit::Milliseconds,
            now.as_secs().saturating_sub(REASONABLE_TIME_WINDOW.as_secs()) as i64,
            now.as_secs().saturating_add(REASONABLE_TIME_WINDOW.as_secs()) as i64,
            1000,
        ),
    ];

    for (unit, lo, hi, scale) in candidates {
        if count_in_range(ca, *lo, *hi) > DURATION_RATIO {
            if *scale > 1 {
                return Some(
                    (col(name.as_str()) * lit(*scale))
                        .cast(DataType::Datetime(*unit, None))
                        .alias(name.as_str()),
                );
            }
            return Some(
                col(name.as_str())
                    .cast(DataType::Datetime(*unit, None))
                    .alias(name.as_str()),
            );
        }
    }

    None
}

fn narrow_int(series: &Series, name: &PlSmallStr) -> Option<Expr> {
    let Ok(s64) = series.cast(&DataType::Int64) else {
        return None;
    };
    let ca = s64.i64().unwrap();
    let (min, max) = match (ca.min(), ca.max()) {
        (Some(min), Some(max)) => (min, max),
        _ => return None,
    };
    let target = narrowest_int_type(min, max);
    if target != *series.dtype() {
        return Some(col(name.as_str()).cast(target).alias(name.as_str()));
    }
    None
}

fn all_column_names(sample: &DataFrame) -> Vec<PlSmallStr> {
    sample
        .get_column_names()
        .iter()
        .map(|n| (*n).clone())
        .collect()
}

fn is_european_locale() -> bool {
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_TIME"))
        .or_else(|_| std::env::var("LC_ALL"))
        .map(|l| {
            l.contains("de_")
                || l.contains("fr_")
                || l.contains("es_")
                || l.contains("it_")
                || l.contains("nl_")
                || l.contains("pt_")
                || l.contains("sv_")
                || l.contains("da_")
                || l.contains("fi_")
                || l.contains("nb_")
                || l.contains("pl_")
                || l.contains("cs_")
                || l.contains("sk_")
                || l.contains("hu_")
                || l.contains("ro_")
                || l.contains("bg_")
                || l.contains("el_")
                || l.contains("ru_")
                || l.contains("uk_")
                || l.contains("tr_")
        })
        .unwrap_or(false)
}

fn count_regex(ca: &StringChunked, re: &Regex) -> (usize, usize) {
    let mut total = 0usize;
    let mut matches = 0usize;

    for val in ca.into_iter().flatten() {
        total += 1;
        let trimmed = val.trim();
        if trimmed.is_empty() || re.is_match(trimmed) {
            matches += 1;
        }
    }

    (total, matches)
}

fn regex_ratio(ca: &StringChunked, re: &Regex) -> f64 {
    let total = ca.len();
    if total == 0 {
        return 0.0;
    }
    let matches = ca
        .into_iter()
        .flatten()
        .filter(|s| re.is_match(s.trim()))
        .count();
    matches as f64 / total as f64
}

fn narrowest_int_type(min: i64, max: i64) -> DataType {
    if min >= 0 {
        if max <= i64::from(u8::MAX) {
            DataType::UInt8
        } else if max <= i64::from(u16::MAX) {
            DataType::UInt16
        } else if max <= i64::from(u32::MAX) {
            DataType::UInt32
        } else {
            DataType::UInt64
        }
    } else if min >= i64::from(i8::MIN) && max <= i64::from(i8::MAX) {
        DataType::Int8
    } else if min >= i64::from(i16::MIN) && max <= i64::from(i16::MAX) {
        DataType::Int16
    } else if min >= i64::from(i32::MIN) && max <= i64::from(i32::MAX) {
        DataType::Int32
    } else {
        DataType::Int64
    }
}

fn count_in_range(ca: &polars::prelude::Int64Chunked, lo: i64, hi: i64) -> f64 {
    let total = ca.len();
    if total == 0 {
        return 0.0;
    }
    let in_range = ca
        .into_iter()
        .flatten()
        .filter(|&v| v >= lo && v <= hi)
        .count() as f64;
    in_range / total as f64
}

fn bool_pair(a: &str, b: &str) -> (Option<String>, Option<String>) {
    let a_true = is_true_like(a);
    let b_true = is_true_like(b);
    let a_false = is_false_like(a);
    let b_false = is_false_like(b);

    if a_true && b_false {
        (Some(a.to_string()), Some(b.to_string()))
    } else if a_false && b_true {
        (Some(b.to_string()), Some(a.to_string()))
    } else {
        (None, None)
    }
}

fn is_true_like(s: &str) -> bool {
    matches!(
        s,
        "true" | "True" | "TRUE" | "yes" | "Yes" | "YES" | "y" | "Y" | "t" | "T" | "1"
    )
}

fn is_false_like(s: &str) -> bool {
    matches!(
        s,
        "false" | "False" | "FALSE" | "no" | "No" | "NO" | "n" | "N" | "f" | "F" | "0"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_of(lf: &LazyFrame) -> DataFrame {
        lf.clone().limit(SAMPLE_SIZE as u32).collect().unwrap()
    }

    fn dtype_of(lf: &LazyFrame, name: &str) -> DataType {
        lf.clone()
            .select([col(name)])
            .limit(1)
            .collect()
            .unwrap()
            .column(name)
            .unwrap()
            .dtype()
            .clone()
    }

    // ---- infer_df ----

    #[test]
    fn infer_df_pipeline() {
        let n = 20i64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as i64;

        let str_nums: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        let dates: Vec<String> = (0..n)
            .map(|i| format!("2024-01-{:02}", (i % 28) + 1))
            .collect();
        let roles: Vec<&str> = (0..n)
            .map(|i| {
                if i % 3 == 0 {
                    "admin"
                } else if i % 3 == 1 {
                    "user"
                } else {
                    "mod"
                }
            })
            .collect();
        let big_ints: Vec<i64> = (0..n).collect();
        let ts: Vec<i64> = (0..n).map(|_| now).collect();

        let lf = df!(
            "str_nums" => str_nums.as_slice(),
            "dates" => dates.as_slice(),
            "roles" => roles.as_slice(),
            "big_ints" => big_ints.as_slice(),
            "ts" => ts.as_slice(),
        )
        .unwrap()
        .lazy();

        let lf = infer_df(lf, &Schema::default(), DateLocale::Auto);

        assert!(dtype_of(&lf, "str_nums").is_integer());
        assert_eq!(dtype_of(&lf, "dates"), DataType::Date);
        assert!(matches!(dtype_of(&lf, "roles"), DataType::Categorical(..)));
        assert!(dtype_of(&lf, "big_ints").is_integer());
        assert_eq!(
            dtype_of(&lf, "ts"),
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
    }

    #[test]
    fn infer_df_respects_schema() {
        let mut schema = Schema::default();
        schema.with_column("str_nums".into(), DataType::String);

        let lf = df!(
            "str_nums" => &["10", "20", "30"],
            "big_ints" => &[1i64, 2, 3],
        )
        .unwrap()
        .lazy();

        let lf = infer_df(lf, &schema, DateLocale::Auto);

        assert_eq!(dtype_of(&lf, "str_nums"), DataType::String);
        assert_eq!(dtype_of(&lf, "big_ints"), DataType::UInt8);
    }

    #[test]
    fn col_mixed_strings_stay_string() {
        let lf = df!("vals" => &["1", "hello", "2", "world"]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("vals"), &sample, DateLocale::Auto);
        assert_eq!(dtype_of(&lf, "vals"), DataType::String);
    }

    #[test]
    fn col_empty_strings_are_convertible() {
        let vals: Vec<&str> = std::iter::once("")
            .chain((0..19).map(|_| "1"))
            .collect();
        let lf = df!("nums" => vals.as_slice()).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("nums"), &sample, DateLocale::Auto);
        assert!(dtype_of(&lf, "nums").is_integer() || dtype_of(&lf, "nums") == DataType::Boolean);
    }

    // ---- infer_column: string -> temporal ----

    #[test]
    fn col_string_to_date_iso() {
        let lf = df!("d" => &["2024-01-15", "2023-12-31", "2020-06-01"])
            .unwrap()
            .lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("d"), &sample, DateLocale::Auto);
        assert_eq!(dtype_of(&lf, "d"), DataType::Date);
    }

    // ---- infer_column: string -> bool ----

    #[test]
    fn col_yes_no_to_bool() {
        let lf = df!("x" => &["yes", "no", "yes", "no"]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("x"), &sample, DateLocale::Auto);
        assert_eq!(dtype_of(&lf, "x"), DataType::Boolean);
    }

    // ---- infer_column: integer -> bool ----

    #[test]
    fn col_zero_one_to_bool() {
        let lf = df!("x" => &[1i64, 0, 1, 0]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("x"), &sample, DateLocale::Auto);
        assert_eq!(dtype_of(&lf, "x"), DataType::Boolean);
    }

    // ---- infer_column: integer -> narrow ----

    #[test]
    fn col_narrow_to_uint8() {
        let lf = df!("x" => &[1i64, 2, 3]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("x"), &sample, DateLocale::Auto);
        assert_eq!(dtype_of(&lf, "x"), DataType::UInt8);
    }

    // ---- infer_column: integer -> datetime ----

    #[test]
    #[test]
    fn col_datetime_from_micros() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as i64;
        let lf = df!("ts" => &[now]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("ts"), &sample, DateLocale::Auto);
        assert_eq!(
            dtype_of(&lf, "ts"),
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
    }

    // ---- infer_column: string -> locale dt ----

    #[test]
    fn col_string_to_locale_dt_us() {
        let lf = df!("ts" => &["02/01/2025, 02:00:00 PM UTC"]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("ts"), &sample, DateLocale::American);
        assert_eq!(
            dtype_of(&lf, "ts"),
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
    }

    #[test]
    fn col_string_to_locale_dt_eu() {
        let lf = df!("ts" => &["01/02/2025, 02:00:00 PM UTC"]).unwrap().lazy();
        let sample = sample_of(&lf);
        let lf = infer_column(lf, &PlSmallStr::from("ts"), &sample, DateLocale::European);
        assert_eq!(
            dtype_of(&lf, "ts"),
            DataType::Datetime(TimeUnit::Microseconds, None)
        );
    }
}
