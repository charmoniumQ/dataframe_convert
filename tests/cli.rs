use std::process::Command;
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dataframe_convert"))
}

fn run(dir: &TempDir, input_csv: &str, ext: &str) -> String {
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, input_csv).unwrap();
    let out = dir.path().join(format!("out.{ext}"));
    let r = bin().arg("cat").arg(&inp).arg(&out).output().unwrap();
    assert!(
        r.status.success(),
        "cat csv→{ext}: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    std::fs::read_to_string(&out).unwrap()
}

#[test]
fn csv_to_csv() {
    let dir = TempDir::new().unwrap();
    let out = run(&dir, "a,b\n1,x\n2,y\n", "csv");
    assert!(out.contains("a,b"), "preserves header: {out}");
    assert!(out.contains("1,x"), "preserves row: {out}");
}

#[test]
fn csv_to_tsv() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,x\n2,y\n").unwrap();
    let out = dir.path().join("out.tsv");
    let r = bin().arg("cat").arg(&inp).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("a\tb"), "uses tab separator: {content:?}");
    assert!(content.contains("1\tx"), "tab in row: {content:?}");
}

#[test]
fn csv_to_parquet() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,x\n2,y\n").unwrap();
    let out = dir.path().join("out.parquet");
    let r = bin().arg("cat").arg(&inp).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(out.exists() && out.metadata().unwrap().len() > 0);
}

#[test]
fn csv_to_json() {
    let dir = TempDir::new().unwrap();
    let out = run(&dir, "a,b\n1,x\n2,y\n", "json");
    assert!(out.contains("\"a\""), "has column name: {out}");
    assert!(out.contains("\"x\""), "has value: {out}");
}

#[test]
fn csv_to_ipc() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,x\n2,y\n").unwrap();
    let out = dir.path().join("out.ipc");
    let r = bin().arg("cat").arg(&inp).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(out.exists() && out.metadata().unwrap().len() > 0);
}

#[test]
fn explicit_csv_separator() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.psv");
    std::fs::write(&inp, "a|b\n1|x\n2|y\n").unwrap();
    let out = dir.path().join("out.json");
    let r = bin()
        .arg("cat")
        .arg("-i")
        .arg("csv:sep=|")
        .arg(&inp)
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("\"a\""), "pipe sep→json: {content}");
}

#[test]
fn explicit_format_flags() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("data.in");
    std::fs::write(&inp, "a,b\n1,x\n2,y\n").unwrap();
    let out = dir.path().join("data.out");
    let r = bin()
        .arg("cat")
        .arg("-i")
        .arg("csv")
        .arg("-o")
        .arg("json")
        .arg(&inp)
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        r.status.success(),
        "explicit formats: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("\"a\""), "explicit csv→json: {content}");
}

#[test]
fn explicit_tsv_format() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,x\n2,y\n").unwrap();
    let out = dir.path().join("out.tsv");
    let r = bin()
        .arg("cat")
        .arg("-i")
        .arg("csv")
        .arg("-o")
        .arg("tsv")
        .arg(&inp)
        .arg(&out)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("a\tb"), "tsv separator: {content:?}");
}

#[test]
fn metadata_command() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,2\n3,4\n").unwrap();
    let r = bin().arg("metadata").arg(&inp).output().unwrap();
    assert!(
        r.status.success(),
        "metadata: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("a"), "column a: {stdout}");
    assert!(stdout.contains("b"), "column b: {stdout}");
}

#[test]
fn metadata_with_dtypes() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,2.5\n3,4.0\n").unwrap();
    let r = bin()
        .arg("metadata")
        .arg("--dtypes")
        .arg("a=float64")
        .arg("--dtypes")
        .arg("b=float64")
        .arg(&inp)
        .output()
        .unwrap();
    assert!(
        r.status.success(),
        "metadata dtypes: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("f64"), "float dtype: {stdout}");
}

#[test]
fn csv_roundtrip_inference() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a,b\n1,x\n2,y\n").unwrap();
    let mid = dir.path().join("mid.json");
    let out = dir.path().join("out.csv");

    let r = bin().arg("cat").arg(&inp).arg(&mid).output().unwrap();
    assert!(
        r.status.success(),
        "csv→json: {}",
        String::from_utf8_lossy(&r.stderr)
    );

    let r = bin().arg("cat").arg(&mid).arg(&out).output().unwrap();
    assert!(
        r.status.success(),
        "json→csv: {}",
        String::from_utf8_lossy(&r.stderr)
    );

    let content = std::fs::read_to_string(&out).unwrap();
    assert!(
        content.contains("a,b") || content.contains("\"a\""),
        "roundtrip: {content}"
    );
}

#[test]
fn bad_format_fails() {
    let dir = TempDir::new().unwrap();
    let inp = dir.path().join("in.csv");
    std::fs::write(&inp, "a\n1\n").unwrap();
    let out = dir.path().join("out.xyz");
    let r = bin().arg("cat").arg(&inp).arg(&out).output().unwrap();
    assert!(!r.status.success(), "unknown extension should fail");
}
