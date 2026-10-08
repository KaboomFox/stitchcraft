//! The StitchCraft parameter registry (layer L0).
//!
//! Every embroidery parameter is declared once, beside the generator that uses it, and everything else
//! that needs to know about parameters — typed structs, validation, VectorCraft plug-in manifests, CLI
//! help, SVG attributes, reference docs, property-test strategies — is generated from that declaration.
//!
//! Status: planned for roadmap step M3.1. Design: `docs/src/design/params.md`.
#![forbid(unsafe_code)]
