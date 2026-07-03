use polars::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub enum InputFormat {
    Csv {
        separator: u8,
        has_header: bool,
        ignore_errors: bool,
        skip_rows: usize,
    },
    Parquet,
    Json {
        ignore_errors: bool,
    },
    Ipc,
    Xlsx {
        has_header: bool,
        ignore_errors: bool,
    },
    Xls {
        has_header: bool,
        ignore_errors: bool,
    },
    Ods {
        has_header: bool,
        ignore_errors: bool,
    },
    Sqlite {
        table: String,
        ignore_errors: bool,
    },
    Duckdb {
        table: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputFormat {
    Csv { separator: u8, emit_header: bool },
    Parquet,
    Json,
    Ipc,
    Xlsx { emit_header: bool },
    Sqlite { table: String },
    Duckdb { table: String },
    Md,
}

impl InputFormat {
    pub fn label(self, path: PlRefPath) -> String {
        let p = path.to_string().to_string();
        match self {
            Self::Duckdb { table } => format!("{p}?table={table}"),
            Self::Sqlite { table, .. } => format!("{p}?table={table}"),
            _ => p,
        }
    }
}
