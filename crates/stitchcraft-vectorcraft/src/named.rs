//! A `.vectorcraft` document with names written into its objects (`REQ-VC-033`): what `stitch plan
//! picture.png --traced` writes, so that VectorCraft's Layers panel shows each traced shape by the name the
//! notices and the report give it (`docs/src/design/image-to-embroidery.md` › Names of shapes).
//!
//! **Why by hand.** The document must stay the one VectorCraft saved but for the names. Read into a JSON
//! tree and written again, its numbers would be written anew, and serde_json's default reader can be a unit
//! in the last place off on VectorCraft's shortest decimals (see `wire`), which would move points. So the
//! text is scanned, and a name goes in after an object's opening brace, or in place of a `null` name, and
//! every other byte stays as it was.
//!
//! **Its own form.** A compressed file is unpacked, named and compressed again, and the data a file keeps
//! after its JSON, behind the marker (`container`), stays as it is.
//!
//! **Which objects.** An object of a layer's or a group's `children` whose numeric `id` is given a name
//! and that has none: the objects StitchCraft's ids name `vc:` and their id. A name the object has is its
//! own, kept (`REQ-AUTO-126`).

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Code, Diagnostic};

use crate::container;

/// Where a name goes in: at `at`, `text`, in place of `len` bytes.
struct Edit {
    at: usize,
    len: usize,
    text: String,
}

/// An object being scanned: whether it is one of a `children` list, its id, and where its name goes.
#[derive(Default)]
struct Object {
    child: bool,
    open: usize,
    first: Option<usize>,
    id: Option<u64>,
    named: bool,
    null: Option<usize>,
    key: Option<String>,
}

/// A container being scanned: an object, or a list under the key it was given.
enum Frame {
    Object(Object),
    List { children: bool },
}

/// `document`, a `.vectorcraft` file's text, with each object of a `children` list whose id is a key of
/// `names` and that has no name given that name, with `budget`'s work: a unit for each byte. Every other
/// byte stays as it was. A document that is not JSON as VectorCraft writes it, or work that runs out, is
/// an error, `SC-E0009` or `SC-E0004`.
pub fn named(document: &[u8], names: &BTreeMap<u64, String>, budget: &Budget) -> Result<Vec<u8>, Diagnostic> {
    let (whole, compressed) = container::whole(document, &mut budget.meter())?;
    // The JSON ends before the marker of the data after it, which stays as it is.
    let end = whole.iter().position(|&b| b == 0).map_or(whole.len(), |nul| nul.saturating_sub(1));
    let (json, after) = whole.split_at_checked(end).unwrap_or((&whole, &[]));
    let mut out = json_named(json, names, budget)?;
    out.extend_from_slice(after);
    Ok(if compressed { container::packed(&out) } else { out })
}

/// The JSON text `document` with names written, as [`named`] writes them.
fn json_named(document: &[u8], names: &BTreeMap<u64, String>, budget: &Budget) -> Result<Vec<u8>, Diagnostic> {
    let broken =
        || Diagnostic::new(Code::InternalCheckFailed, "The traced document's names can't be written: it isn't JSON as VectorCraft writes it.");
    budget.meter().charge(u64::try_from(document.len()).unwrap_or(u64::MAX)).map_err(|exhausted| exhausted.diagnostic(budget, None))?;
    let text = std::str::from_utf8(document).map_err(|_| broken())?;
    let mut edits = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut i = 0;
    while let Some(&byte) = document.get(i) {
        match byte {
            b'{' => {
                let child = matches!(stack.last(), Some(Frame::List { children: true }));
                stack.push(Frame::Object(Object { child, open: i, ..Object::default() }));
                i += 1;
            }
            b'[' => {
                let children = matches!(stack.last(), Some(Frame::Object(object)) if object.key.as_deref() == Some("children"));
                stack.push(Frame::List { children });
                i += 1;
            }
            b'}' => {
                let Some(Frame::Object(object)) = stack.pop() else { return Err(broken()) };
                edits.extend(edit(&object, i, text, names));
                i += 1;
            }
            b']' => {
                let Some(Frame::List { .. }) = stack.pop() else { return Err(broken()) };
                i += 1;
            }
            b'"' => {
                let end = string_end(document, i).ok_or_else(broken)?;
                let content = text.get(i + 1..end).ok_or_else(broken)?;
                if let Some(Frame::Object(object)) = stack.last_mut() {
                    object.first.get_or_insert(i);
                    match &object.key {
                        // A key, before its colon.
                        None => object.key = Some(content.to_string()),
                        Some(key) if key == "name" => object.named = true,
                        Some(_) => {}
                    }
                }
                i = end + 1;
            }
            b',' => {
                if let Some(Frame::Object(object)) = stack.last_mut() {
                    object.key = None;
                }
                i += 1;
            }
            b':' | b' ' | b'\n' | b'\r' | b'\t' => i += 1,
            _ => {
                let end = document.get(i..).and_then(|rest| rest.iter().position(|b| matches!(b, b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t')));
                let end = end.map_or(document.len(), |n| i + n);
                let token = text.get(i..end).ok_or_else(broken)?;
                if let Some(Frame::Object(object)) = stack.last_mut() {
                    match object.key.as_deref() {
                        Some("id") => object.id = token.parse().ok(),
                        Some("name") if token == "null" => object.null = Some(i),
                        Some("name") => object.named = true,
                        _ => {}
                    }
                }
                i = end;
            }
        }
    }
    if !stack.is_empty() {
        return Err(broken());
    }
    edits.sort_by_key(|edit| edit.at);
    let mut out = Vec::with_capacity(document.len() + edits.iter().map(|edit| edit.text.len()).sum::<usize>());
    let mut from = 0;
    for edit in edits {
        out.extend_from_slice(document.get(from..edit.at).ok_or_else(broken)?);
        out.extend_from_slice(edit.text.as_bytes());
        from = edit.at + edit.len;
    }
    out.extend_from_slice(document.get(from..).ok_or_else(broken)?);
    Ok(out)
}

/// The edit that names `object`, closed at `close` in `text`, by `names`, if it is to be named.
fn edit(object: &Object, close: usize, text: &str, names: &BTreeMap<u64, String>) -> Option<Edit> {
    let name = object.id.filter(|_| object.child && !object.named).and_then(|id| names.get(&id))?;
    let quoted = serde_json::to_string(name).ok()?;
    if let Some(at) = object.null {
        return Some(Edit { at, len: "null".len(), text: quoted });
    }
    // After the opening brace and the white space before the first key, which the name's own key repeats, or
    // a space.
    match object.first {
        Some(first) => {
            let space = text.get(object.open + 1..first).filter(|space| !space.is_empty()).unwrap_or(" ");
            Some(Edit { at: first, len: 0, text: format!("\"name\": {quoted},{space}") })
        }
        None => Some(Edit { at: close, len: 0, text: format!("\"name\": {quoted}") }),
    }
}

/// Where the JSON string that opens at `open` in `bytes` ends: its closing quote.
fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut i = open + 1;
    loop {
        match bytes.get(i)? {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(document: &str, names: &[(u64, &str)]) -> Result<String, Diagnostic> {
        let names = names.iter().map(|(id, name)| (*id, (*name).to_string())).collect();
        named(document.as_bytes(), &names, &Budget::DEFAULT).map(|bytes| String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn req_vc_033_a_name_goes_into_each_object_of_a_children_list_without_one_and_nothing_else_changes() {
        let document = r#"{"layers": [{"id": 1, "kind": {"children": [
            {"id": 3, "kind": {"children": [{"id": 4}]}},
            {"name": null, "id": 6, "p": [0.30000000000000004, -1e-7, "a\"}"]},
            {"id": 9, "name": "Lakeside"},
            {}
        ]}}], "swatches": [{"id": 3}]}"#;
        let names = [(3, "Left out 1, White"), (4, "Red \"fill\" (1)"), (6, "Fill 2, Red"), (9, "Fill 1, Blue"), (1, "Layer")];
        let want = document
            .replace(r#"{"id": 3, "kind""#, r#"{"name": "Left out 1, White", "id": 3, "kind""#)
            .replace(r#"{"id": 4}"#, r#"{"name": "Red \"fill\" (1)", "id": 4}"#)
            .replace(r#"{"name": null, "id": 6"#, r#"{"name": "Fill 2, Red", "id": 6"#);
        assert_eq!(written(document, &names).unwrap(), want);
        // The white space before the first key is kept, and an empty object takes its name alone.
        let names = [(2, "Fill 1, Red"), (5, "Fill 1, Blue")];
        let pretty = "{\"children\": [\n  {\n    \"id\": 2\n  }, {\"id\": 5}]}";
        assert_eq!(
            written(pretty, &names).unwrap(),
            "{\"children\": [\n  {\n    \"name\": \"Fill 1, Red\",\n    \"id\": 2\n  }, {\"name\": \"Fill 1, Blue\", \"id\": 5}]}"
        );
        assert_eq!(written(r#"{"children": [{}]}"#, &names).unwrap(), r#"{"children": [{}]}"#);
        // An object with a name of another kind keeps it, and one with other texts is named.
        let kinds = r#"{"children": [{"id": 2, "name": 12}, {"id": 5, "type": "path", "on": true}]}"#;
        let want = r#"{"children": [{"id": 2, "name": 12}, {"name": "Fill 1, Blue", "id": 5, "type": "path", "on": true}]}"#;
        assert_eq!(written(kinds, &names).unwrap(), want);
    }

    #[test]
    fn req_vc_033_a_compressed_document_stays_compressed_and_the_data_after_its_json_stays() {
        use std::io::{Read, Write};
        let names = BTreeMap::from([(2, "Fill 1, Red".to_string())]);
        let document = b"{\"children\": [{\"id\": 2}]}\n\0VCBLOBS\0\xff\x00data";
        let want = b"{\"children\": [{\"name\": \"Fill 1, Red\", \"id\": 2}]}\n\0VCBLOBS\0\xff\x00data";
        assert_eq!(named(document, &names, &Budget::DEFAULT).unwrap(), want);
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(document).unwrap();
        let packed = named(&encoder.finish().unwrap(), &names, &Budget::DEFAULT).unwrap();
        let mut unpacked = Vec::new();
        flate2::read::GzDecoder::new(packed.as_slice()).read_to_end(&mut unpacked).unwrap();
        assert_eq!(unpacked, want);
        // A damaged stream is refused.
        assert_eq!(named(&[0x1f, 0x8b, 0], &names, &Budget::DEFAULT).map_err(|e| e.code), Err(Code::VcUnreadable));
    }

    #[test]
    fn a_document_that_isn_t_json_as_vectorcraft_writes_it_is_an_error() {
        for broken in [r#"{"children": [}"#, r#"{"children": {]"#, r#"{"children": ["#, r#"{"a": "b"#] {
            assert_eq!(written(broken, &[]).map_err(|e| e.code), Err(Code::InternalCheckFailed), "{broken}");
        }
        assert_eq!(named(&[0xff], &BTreeMap::new(), &Budget::DEFAULT).map_err(|e| e.code), Err(Code::InternalCheckFailed));
        let budget = Budget { max_work: 3, ..Budget::DEFAULT };
        assert_eq!(named(b"{\"a\": 1}", &BTreeMap::new(), &budget).map_err(|e| e.code), Err(Code::BudgetExhausted));
    }
}
