// The tests read fixtures, write folders of their own and run curl.
#![allow(clippy::disallowed_methods)]

use stitchcraft_testkit::temp::TempDir;

use super::*;

/// A fixture of the cut-out's, by name (`conformance/fixtures/cutout/`).
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../conformance/fixtures/cutout/{name}", env!("CARGO_MANIFEST_DIR")))
}

/// `path` as an address curl fetches from the disk: `file:///C:/…` on Windows, `file:///…` elsewhere.
fn address(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    format!("file://{}{path}", if path.starts_with('/') { "" } else { "/" })
}

/// The pin of `source` under the name `file`, published at its own address.
fn pin_of(file: &str, source: &Path) -> Pin {
    let bytes = std::fs::read(source).unwrap();
    Pin { file: file.to_string(), address: address(source), size: bytes.len() as u64, sha256: sha256(&bytes) }
}

/// Pins of the 3 stand-ins (`make_stand_ins.py`), published where the fixtures are.
fn stand_ins() -> Pins {
    Pins {
        u2netp: pin_of("u2netp.onnx", &fixture("stand-in-u2netp.onnx")),
        encoder: pin_of("mobilesam-encoder.onnx", &fixture("stand-in-encoder.onnx")),
        decoder: pin_of("mobilesam-decoder.onnx", &fixture("stand-in-decoder.onnx")),
    }
}

/// The names in `folder`, sorted.
fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

#[test]
fn req_cli_016_the_repository_pins_the_3_files_of_its_release_by_address_size_and_sha256() {
    let pins = Pins::pinned().unwrap();
    assert_eq!(pins.all().map(|pin| pin.file.as_str()), ["u2netp.onnx", "mobilesam-encoder.onnx", "mobilesam-decoder.onnx"]);
    for pin in pins.all() {
        assert_eq!(pin.address, format!("https://github.com/KaboomFox/stitchcraft/releases/download/cutout-models-1/{}", pin.file));
    }
    // 49.0 MB together, as the design says.
    let total: u64 = pins.all().iter().map(|pin| pin.size).sum();
    assert_eq!((total + 50_000) / 100_000, 490, "{total}");
}

#[test]
fn req_cli_016_the_folder_is_the_one_stitchcraft_models_names_or_else_in_the_cache_folder() {
    let env = |vars: &'static [(&'static str, &'static str)]| {
        move |name: &str| vars.iter().find(|(var, _)| *var == name).map(|(_, value)| OsString::from(value))
    };
    const ALL: [(&str, &str); 4] = [("STITCHCRAFT_MODELS", "/models"), ("XDG_CACHE_HOME", "/xdg"), ("HOME", "/home/u"), ("LOCALAPPDATA", "C:/Local")];
    let all: &'static [(&str, &str)] = &ALL;
    assert_eq!(folder_from(&env(all)), Some(PathBuf::from("/models")));
    let cache = PathBuf::from("stitchcraft").join("models");
    assert_eq!(folder_from(&env(&all[1..])), Some(PathBuf::from("/xdg").join(&cache)));
    assert_eq!(folder_from(&env(&all[2..])), Some(PathBuf::from("/home/u/.cache").join(&cache)));
    assert_eq!(folder_from(&env(&all[3..])), Some(PathBuf::from("C:/Local").join(&cache)));
    assert_eq!(folder_from(&env(&[])), None);
    // A variable set to nothing names nothing.
    assert_eq!(folder_from(&env(&[("STITCHCRAFT_MODELS", ""), ("HOME", "/home/u")])), Some(PathBuf::from("/home/u/.cache").join(&cache)));
    assert!(folder().is_some() || std::env::var_os("HOME").is_none());
}

#[test]
fn req_cli_016_files_missing_or_changed_in_the_folder_are_fetched_with_curl_and_checked() {
    let dir = TempDir::new("cutout-pins-fetched");
    let folder = dir.join("models");
    let pins = stand_ins();
    // The folder doesn't exist yet: it is made, and each file fetched into it.
    let first = ready(&pins, Some(&folder)).unwrap();
    assert_eq!(first.u2netp, std::fs::read(fixture("stand-in-u2netp.onnx")).unwrap());
    assert_eq!(first.encoder, std::fs::read(fixture("stand-in-encoder.onnx")).unwrap());
    assert_eq!(first.decoder, std::fs::read(fixture("stand-in-decoder.onnx")).unwrap());
    assert_eq!(names(&folder), ["mobilesam-decoder.onnx", "mobilesam-encoder.onnx", "u2netp.onnx"]);
    // A file changed in the folder, longer or shorter than its pin, is fetched again, and one that matches
    // is used as it is.
    std::fs::write(folder.join("u2netp.onnx"), b"changed").unwrap();
    std::fs::write(folder.join("mobilesam-encoder.onnx"), vec![0; 10_000]).unwrap();
    let again = ready(&pins, Some(&folder)).unwrap();
    assert_eq!((again.u2netp, again.encoder), (first.u2netp, first.encoder));
    assert_eq!(names(&folder), ["mobilesam-decoder.onnx", "mobilesam-encoder.onnx", "u2netp.onnx"]);
}

#[test]
fn diag_sc_e1201_a_file_that_cant_be_fetched_names_each_files_address_size_and_sha256_and_the_folder() {
    let dir = TempDir::new("cutout-pins-missing");
    let folder = dir.join("models");
    let missing = dir.join("nowhere").join("u2netp.onnx");
    let pins = Pins { u2netp: Pin { address: address(&missing), ..stand_ins().u2netp }, ..stand_ins() };
    let said = ready(&pins, Some(&folder)).unwrap_err();
    assert_eq!(said.code, Code::CutoutModelsNotReady);
    let start =
        format!("error SC-E1201: The subject cut-out's models are not ready in {}: curl could not fetch {} (", folder.display(), pins.u2netp.address);
    assert!(said.to_string().starts_with(&start), "{said}");
    let files = pins.all().map(|pin| format!("{}, {} bytes, SHA-256 {}, from {}", pin.file, grouped(pin.size), pin.sha256, pin.address)).join("; ");
    assert!(
        said.message.ends_with(&format!("). Put these files in {}, or in the folder STITCHCRAFT_MODELS names: {files}.", folder.display())),
        "{}",
        said.message
    );
    assert_eq!(
        said.fix.map(|fix| fix.describe()).as_deref(),
        Some("Fetch the files elsewhere and put them in the folder, or plan the picture without --subject, as before.")
    );
    // The 2 files that could be fetched are in place, and nothing else is left.
    assert_eq!(names(&folder), ["mobilesam-decoder.onnx", "mobilesam-encoder.onnx"]);
}

#[test]
fn diag_sc_e1201_a_file_fetched_whose_sha256_or_size_differs_is_refused_and_not_kept() {
    let dir = TempDir::new("cutout-pins-differ");
    let folder = dir.join("models");
    let other = "0".repeat(64);
    let pins = Pins { decoder: Pin { sha256: other.clone(), ..stand_ins().decoder }, ..stand_ins() };
    let said = ready(&pins, Some(&folder)).unwrap_err();
    let sum = stand_ins().decoder.sha256;
    assert!(
        said.message.contains(&format!(": mobilesam-decoder.onnx was fetched, and its SHA-256 is {sum}, not {other}. Put these")),
        "{}",
        said.message
    );
    assert_eq!(names(&folder), ["mobilesam-encoder.onnx", "u2netp.onnx"]);
    // Fetched shorter or longer than its pin, it is said so.
    let size = stand_ins().decoder.size;
    for (pinned, why) in
        [(size + 1, format!("it is {size} bytes, not {}", size + 1)), (size - 1, format!("it is larger than its {} bytes", size - 1))]
    {
        let pins = Pins { decoder: Pin { size: pinned, ..stand_ins().decoder }, ..stand_ins() };
        let said = ready(&pins, Some(&folder)).unwrap_err();
        assert!(said.message.contains(&format!("mobilesam-decoder.onnx was fetched, and {why}. Put these")), "{}", said.message);
    }
    // Every reason is said, in the files' order.
    let pins =
        Pins { u2netp: Pin { sha256: other.clone(), ..stand_ins().u2netp }, decoder: Pin { sha256: other, ..stand_ins().decoder }, ..stand_ins() };
    let said = ready(&pins, Some(&folder)).unwrap_err().message;
    let (first, second) = (said.find("u2netp.onnx was fetched").unwrap(), said.find("mobilesam-decoder.onnx was fetched").unwrap());
    assert!(first < second && said[first..second].ends_with("; "), "{said}");
}

#[test]
fn diag_sc_e1201_without_a_folder_the_message_asks_for_one() {
    let said = ready(&stand_ins(), None).unwrap_err();
    assert!(
        said.message.starts_with("The subject cut-out's models are not ready: there is no folder for them: set STITCHCRAFT_MODELS to one. Put these files in a folder, or in the folder STITCHCRAFT_MODELS names: u2netp.onnx, "),
        "{}",
        said.message
    );
    // A folder that can't be made is said too: a file stands where it would be.
    let dir = TempDir::new("cutout-pins-blocked");
    std::fs::write(dir.join("models"), b"a file").unwrap();
    let said = ready(&stand_ins(), Some(&dir.join("models"))).unwrap_err();
    assert!(said.message.contains(": the folder could not be made ("), "{}", said.message);
}

#[test]
fn a_file_far_larger_than_its_pin_is_read_only_to_a_byte_past_it() {
    let dir = TempDir::new("cutout-pins-large");
    let path = dir.join("large.onnx");
    std::fs::write(&path, vec![7; 1_000]).unwrap();
    assert_eq!(read(&path, 10).unwrap().len(), 11);
    assert_eq!(read(&path, 1_000).unwrap().len(), 1_000);
    assert!(read(&dir.join("absent.onnx"), 10).is_err());
}

#[test]
fn pins_that_dont_read_are_a_bug_of_the_repositorys() {
    for (text, why) in [("", "missing field `model`"), ("[model.u2netp]\nfile = \"u2netp.onnx\"", "missing field")] {
        let said = Pins::parse(text).unwrap_err();
        assert_eq!(said.code, Code::InternalCheckFailed);
        assert!(
            said.message.starts_with("The subject cut-out's models are not pinned as it reads them: ") && said.message.contains(why),
            "{}",
            said.message
        );
    }
    let pinned = |file: &str, sha256: &str| {
        let pin = format!("file = \"{file}\"\naddress = \"https://example.org/{file}\"\nsize = 1\nsha256 = \"{sha256}\"\n");
        format!("[model.u2netp]\n{pin}[model.encoder]\n{pin}[model.decoder]\n{pin}")
    };
    let good = "ab".repeat(32);
    assert!(Pins::parse(&pinned("u2netp.onnx", &good)).is_ok());
    for (file, sha256) in [
        ("u2netp.onnx", "AB".repeat(32)),
        ("u2netp.onnx", "ab".repeat(31)),
        ("u2netp.onnx", "xy".repeat(32)),
        ("models/u2netp.onnx", good.clone()),
        ("", good),
    ] {
        let said = Pins::parse(&pinned(file, &sha256)).unwrap_err();
        assert_eq!(
            said.message,
            format!("The subject cut-out's models are not pinned as it reads them: {file} is pinned with the SHA-256 `{sha256}`.")
        );
    }
    assert_eq!(grouped(4_574_861), "4,574,861");
    assert_eq!(
        (grouped(0), grouped(999), grouped(1_000), grouped(123_456)),
        ("0".to_string(), "999".to_string(), "1,000".to_string(), "123,456".to_string())
    );
    // Each fetch's temporary file is its own, within one process too.
    let (first, second) = (temporary(Path::new("models/u2netp.onnx")), temporary(Path::new("models/u2netp.onnx")));
    let prefix = format!("models/u2netp.onnx.{}-", std::process::id());
    assert!(first.to_string_lossy().starts_with(&prefix) && first.to_string_lossy().ends_with(".partial"), "{}", first.display());
    assert_ne!(first, second);
}
