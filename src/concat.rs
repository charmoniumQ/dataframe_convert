use anyhow::{Context, Result};
use polars::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub enum Incompatibility {
    ExtraColumn {
        source: String,
        column: String,
    },
    MissingColumn {
        source: String,
        column: String,
    },
    DtypeMismatch {
        source: String,
        column: String,
        dtype: DataType,
        expected_dtype: DataType,
    },
}

pub fn debug_concat(
    lfs: &mut [(impl AsRef<str>, LazyFrame)],
    union_args: UnionArgs,
) -> Result<Result<LazyFrame, Vec<Incompatibility>>> {
    let schemas: Vec<(String, SchemaRef)> = lfs
        .iter_mut()
        .map(|(label, lf)| {
            let label = label.as_ref();
            lf.collect_schema()
                .with_context(|| format!("failed to collect schema for '{label}'"))
                .map(|s| (label.to_string(), s))
        })
        .collect::<Result<_>>()?;

    let mut incompatibilities: Vec<Incompatibility> = Vec::new();

    if schemas.is_empty() {
        return Ok(Ok(LazyFrame::default()));
    }

    let ref_dtypes: BTreeMap<String, DataType> = schemas[0]
        .1
        .iter()
        .map(|(col, dt)| (col.to_string(), dt.clone()))
        .collect();

    // Non-diagonal: column sets must match exactly.
    if !union_args.diagonal {
        let ref_cols: BTreeSet<String> = ref_dtypes.keys().cloned().collect();
        for (label, schema) in &schemas[1..] {
            let cols: BTreeSet<String> = schema.iter().map(|(n, _)| n.to_string()).collect();
            for col in ref_cols.difference(&cols) {
                incompatibilities.push(Incompatibility::MissingColumn {
                    source: label.to_string(),
                    column: col.clone(),
                });
            }
            for col in cols.difference(&ref_cols) {
                incompatibilities.push(Incompatibility::ExtraColumn {
                    source: label.to_string(),
                    column: col.clone(),
                });
            }
        }
    }

    // Dtype compatibility: compare every subsequent frame against the reference.
    for (label, schema) in &schemas[1..] {
        for (col, dt) in schema.iter() {
            if let Some(ref_dt) = ref_dtypes.get(col.as_str())
                && dt != ref_dt
                && (!union_args.to_supertypes
                    || polars_core::utils::try_get_supertype(dt, ref_dt).is_err())
            {
                incompatibilities.push(Incompatibility::DtypeMismatch {
                    source: label.to_string(),
                    column: col.to_string(),
                    dtype: dt.clone(),
                    expected_dtype: ref_dt.clone(),
                });
            }
        }
    }

    if incompatibilities.is_empty() {
        let frames: Vec<LazyFrame> = lfs.iter().map(|(_, lf)| lf.clone()).collect();
        Ok(Ok(concat(&frames, union_args)?))
    } else {
        Ok(Err(incompatibilities))
    }
}

pub fn concat_lf_diagonal(lfs: &[(impl AsRef<str>, LazyFrame)]) -> Result<LazyFrame> {
    let mut bufs: Vec<(String, LazyFrame)> = lfs
        .iter()
        .map(|(label, lf)| (label.as_ref().to_string(), lf.clone()))
        .collect();
    match debug_concat(
        &mut bufs,
        UnionArgs {
            to_supertypes: true,
            ..Default::default()
        },
    )? {
        Ok(lf) => Ok(lf),
        Err(errs) => anyhow::bail!("concat incompatibilities: {errs:?}",),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u8_u16_promotes_to_u16() {
        let a = df!("x" => &[1u8, 2, 3]).unwrap().lazy();
        let b = df!("x" => &[1000u16, 2000u16]).unwrap().lazy();
        let result = concat_lf_diagonal(&[("a", a), ("b", b)])
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(result.column("x").unwrap().dtype(), &DataType::UInt16);
        assert_eq!(result.height(), 5);
    }

    #[test]
    fn u32_i32_promotes_to_i64() {
        let a = df!("v" => &[1u32, 2u32, 3u32]).unwrap().lazy();
        let b = df!("v" => &[1i32, 2i32, 3i32]).unwrap().lazy();
        let result = concat_lf_diagonal(&[("a", a), ("b", b)])
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(result.column("v").unwrap().dtype(), &DataType::Int64);
    }

    #[test]
    fn int_float_promotes_to_float() {
        let a = df!("y" => &[1i64, 2, 3]).unwrap().lazy();
        let b = df!("y" => &[1.5f64, 3.14f64]).unwrap().lazy();
        let result = concat_lf_diagonal(&[("a", a), ("b", b)])
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(result.column("y").unwrap().dtype(), &DataType::Float64);
    }

    #[test]
    fn same_type_no_promotion() {
        let a = df!("x" => &[1i64, 2]).unwrap().lazy();
        let b = df!("x" => &[3i64, 4]).unwrap().lazy();
        let result = concat_lf_diagonal(&[("a", a), ("b", b)])
            .unwrap()
            .collect()
            .unwrap();
        assert_eq!(result.column("x").unwrap().dtype(), &DataType::Int64);
        assert_eq!(result.height(), 4);
    }
}
