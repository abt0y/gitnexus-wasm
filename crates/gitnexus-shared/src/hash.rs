//! Content hashing for incremental update detection (Task 7)
//!
//! Uses a pure-Rust FNV-1a 64-bit hash (no native deps, WASM-friendly).
//! For production you'd swap in sha2 = { version = "0.10", features = ["oid"] }
//! with the wasm32 target enabled.

/// Compute a stable 64-bit FNV-1a hash of `content`.
///
/// FNV-1a is not cryptographically secure, but it is:
/// - fast in Rust/WASM (no intrinsics, no C deps)
/// - deterministic across platforms
/// - good enough for change detection
pub fn hash_content(content: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for byte in content.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x00000100000001b3); // FNV prime
    }
    format!("{:016x}", hash)
}

/// Combine file path + content into a single hash.
/// Prevents identical files at different paths from sharing a cache entry.
pub fn hash_file(path: &str, content: &str) -> String {
    hash_content(&format!("{}\x00{}", path, content))
}

/// Given a map of `path → stored_hash` and an incoming list of `(path, content)`,
/// partition into (changed, unchanged, new) path lists.
pub fn diff_hashes<'a>(
    stored:   &std::collections::HashMap<String, String>,
    incoming: &'a [(String, String)],
) -> DiffResult<'a> {
    let mut changed   = Vec::new();
    let mut unchanged = Vec::new();
    let mut new_files = Vec::new();

    for (path, content) in incoming {
        let new_hash = hash_file(path, content);
        match stored.get(path.as_str()) {
            Some(old_hash) if old_hash == &new_hash => unchanged.push(path.as_str()),
            Some(_) => changed.push((path.as_str(), content.as_str(), new_hash)),
            None    => new_files.push((path.as_str(), content.as_str(), new_hash)),
        }
    }

    DiffResult { changed, unchanged, new_files }
}

pub struct DiffResult<'a> {
    /// Files whose content hash differs from stored.
    pub changed:   Vec<(&'a str, &'a str, String)>,
    /// Files with identical hashes — skip re-parsing.
    pub unchanged: Vec<&'a str>,
    /// Files not previously seen.
    pub new_files: Vec<(&'a str, &'a str, String)>,
}

impl<'a> DiffResult<'a> {
    /// All files that need to be re-parsed (changed ∪ new).
    pub fn needs_parse(&self) -> impl Iterator<Item = (&str, &str)> {
        self.changed.iter().map(|(p, c, _)| (*p, *c))
            .chain(self.new_files.iter().map(|(p, c, _)| (*p, *c)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_hash() {
        assert_eq!(hash_content("hello"), hash_content("hello"));
    }

    #[test]
    fn different_content_different_hash() {
        assert_ne!(hash_content("hello"), hash_content("world"));
    }

    #[test]
    fn path_matters() {
        assert_ne!(hash_file("a.ts", "x"), hash_file("b.ts", "x"));
    }
}
