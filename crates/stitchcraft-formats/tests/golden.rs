//! REQ-FMT-001: canonical plans encode to the committed golden files, byte for byte, in every format.
//!
//! A changed golden file is a changed machine file: it is re-blessed only on purpose, with
//! `STITCHCRAFT_BLESS=1 cargo test -p stitchcraft-formats --test golden`, in a PR that explains why
//! (`docs/src/design/conformance.md`).

use std::path::PathBuf;

use stitchcraft_formats::encode;
use stitchcraft_plan::FormatId;
use stitchcraft_testkit::plans;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/golden/formats")
}

#[test]
fn req_fmt_001_canonical_plans_match_their_golden_files() {
    let bless = std::env::var_os("STITCHCRAFT_BLESS").is_some();
    for (name, plan) in plans::canonical() {
        for format in FormatId::ALL {
            let bytes = encode(&plan, *format, name).unwrap();
            let path = golden_dir().join(format!("{name}.{}", format.extension()));
            if bless {
                std::fs::write(&path, &bytes).unwrap();
                continue;
            }
            let golden = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e} (bless with STITCHCRAFT_BLESS=1)", path.display()));
            if golden != bytes {
                let first = golden.iter().zip(&bytes).position(|(a, b)| a != b).unwrap_or(golden.len().min(bytes.len()));
                panic!("{}: differs from the golden file at byte {first} (golden {} bytes, now {})", path.display(), golden.len(), bytes.len());
            }
        }
    }
}
