# LocateAnything Backend

The native package is `org.annotagent.locate-anything-rust`. It declares OpenVocabularyDetection
and PhraseGrounding contracts but is `UnsupportedPlatform`: the audited upstream release has no
complete supported Rust-callable model runtime. AnnotAgent does not fall back to an external worker
or present fixture transport as inference.

See [LocateAnything Rust Plugin](LOCATE_ANYTHING_RUST_PLUGIN.md).
