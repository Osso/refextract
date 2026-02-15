fn main() {
    // Force recompilation when knowledge base files change.
    // include_str! embeds these at compile time in kb.rs.
    // Without this, cargo won't detect KB file changes.
    for kb in &[
        "kbs/journal-titles.kb",
        "kbs/report-numbers.kb",
        "kbs/collaborations.kb",
    ] {
        println!("cargo::rerun-if-changed={kb}");
    }
    // Set an env var so that changes to KB files trigger recompilation
    // of the crate (not just re-running the build script).
    let hash = kb_hash();
    println!("cargo::rustc-env=KB_HASH={hash}");
}

fn kb_hash() -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for kb in &[
        "kbs/journal-titles.kb",
        "kbs/report-numbers.kb",
        "kbs/collaborations.kb",
    ] {
        if let Ok(contents) = std::fs::read_to_string(kb) {
            contents.hash(&mut hasher);
        }
    }
    hasher.finish()
}
