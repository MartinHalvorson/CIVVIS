//! Comparator sorts that share one std sort per element type in the browser build.
//!
//! Every `sort_by` / `sort_by_key` call with its own closure monomorphizes a
//! complete copy of std's stable driftsort or unstable ipnsort for that
//! closure. Measured on the published module (2026-11-18, `241674c`): 3.23 MB
//! of its 17 MB of code was 535 such copies, up to 38 KB each, and 79% of
//! those bytes never ran in two full games plus a play/setup/battle sweep.
//!
//! [`SharedSort`] keeps the call sites reading like std's and changes only
//! what the browser build instantiates. On `wasm32` the comparator crosses a
//! `&mut dyn FnMut` into one `#[inline(never)]` std sort per element type, so
//! the module carries one copy per type instead of one per closure. That
//! measured −1.92 MB raw and −282 KB brotli, with identical trace and save
//! hashes on full games and no measurable change in engine time.
//!
//! The order is identical by construction, not by test luck: the same std
//! algorithm runs on the same slice and asks the same `is_less` question.
//! `sort_by` decides `compare(a, b) == Less` and `sort_by_key` decides
//! `f(a) < f(b)`, exactly as std's own implementations do, so even an
//! unstable sort settles ties the same way.
//!
//! Native builds call std directly and are unchanged. Native code size is not
//! scarce, and an indirect call per comparison is a cost the simulator does
//! not need to pay there.
//!
//! ⚠ A std comparator sort in shipping code brings its private copy back.
//! The guard at the bottom of this file fails on one; test code (test files
//! and every `#[cfg(test)]` item) and the native binaries (`src/main.rs`,
//! `src/bin/`) are free to use std's.

use core::cmp::Ordering;

/// Comparator sorts that instantiate std's sort once per element type on
/// `wasm32`. Import it and call `sort_by_shared` where you would call
/// `sort_by`; the result is the same.
pub trait SharedSort<T> {
    /// [`slice::sort_by`].
    fn sort_by_shared<F: FnMut(&T, &T) -> Ordering>(&mut self, compare: F);
    /// [`slice::sort_by_key`].
    fn sort_by_key_shared<K: Ord, F: FnMut(&T) -> K>(&mut self, key: F);
    /// [`slice::sort_unstable_by`].
    fn sort_unstable_by_shared<F: FnMut(&T, &T) -> Ordering>(&mut self, compare: F);
    /// [`slice::sort_unstable_by_key`].
    fn sort_unstable_by_key_shared<K: Ord, F: FnMut(&T) -> K>(&mut self, key: F);
}

#[cfg(any(target_arch = "wasm32", test))]
#[inline(never)]
fn stable<T>(slice: &mut [T], compare: &mut dyn FnMut(&T, &T) -> Ordering) {
    slice.sort_by(|a, b| compare(a, b));
}

#[cfg(any(target_arch = "wasm32", test))]
#[inline(never)]
fn unstable<T>(slice: &mut [T], compare: &mut dyn FnMut(&T, &T) -> Ordering) {
    slice.sort_unstable_by(|a, b| compare(a, b));
}

/// std's `sort_by_key` sorts on `f(a).lt(&f(b))`. Answering `Less` exactly
/// when that holds asks the shared sort the same question.
#[cfg(any(target_arch = "wasm32", test))]
#[inline(always)]
fn key_order<K: Ord>(a: K, b: K) -> Ordering {
    if a.lt(&b) {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

#[cfg(target_arch = "wasm32")]
impl<T> SharedSort<T> for [T] {
    #[inline]
    fn sort_by_shared<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        stable(self, &mut compare);
    }
    #[inline]
    fn sort_by_key_shared<K: Ord, F: FnMut(&T) -> K>(&mut self, mut key: F) {
        stable(self, &mut |a: &T, b: &T| key_order(key(a), key(b)));
    }
    #[inline]
    fn sort_unstable_by_shared<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        unstable(self, &mut compare);
    }
    #[inline]
    fn sort_unstable_by_key_shared<K: Ord, F: FnMut(&T) -> K>(&mut self, mut key: F) {
        unstable(self, &mut |a: &T, b: &T| key_order(key(a), key(b)));
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> SharedSort<T> for [T] {
    #[inline(always)]
    fn sort_by_shared<F: FnMut(&T, &T) -> Ordering>(&mut self, compare: F) {
        self.sort_by(compare);
    }
    #[inline(always)]
    fn sort_by_key_shared<K: Ord, F: FnMut(&T) -> K>(&mut self, key: F) {
        self.sort_by_key(key);
    }
    #[inline(always)]
    fn sort_unstable_by_shared<F: FnMut(&T, &T) -> Ordering>(&mut self, compare: F) {
        self.sort_unstable_by(compare);
    }
    #[inline(always)]
    fn sort_unstable_by_key_shared<K: Ord, F: FnMut(&T) -> K>(&mut self, key: F) {
        self.sort_unstable_by_key(key);
    }
}

#[cfg(test)]
mod tests {
    use super::SharedSort;
    use std::path::{Path, PathBuf};

    /// The std methods whose per-closure copies this module exists to avoid.
    const STD_COMPARATOR_SORTS: [&str; 4] = [
        ".sort_by(",
        ".sort_by_key(",
        ".sort_unstable_by(",
        ".sort_unstable_by_key(",
    ];

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("src/ is readable") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    /// Whether a file is test code in its entirety, by the crate's naming.
    fn is_test_file(path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        name == "tests.rs"
            || name.ends_with("_tests.rs")
            || name.ends_with("_test.rs")
            || path.components().any(|part| part.as_os_str() == "tests")
    }

    /// The source with comments and string/char literals turned into spaces.
    /// Newlines survive, so line numbers do too, and braces inside a format
    /// string or a `'{'` cannot unbalance the item scan below.
    fn blank_literals(text: &str) -> Vec<u8> {
        let bytes = text.as_bytes();
        let mut out = bytes.to_vec();
        let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
            for byte in &mut out[from..to] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
        };
        let identifier = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            if rest.starts_with(b"//") {
                let end = rest
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map_or(bytes.len(), |n| i + n);
                blank(&mut out, i, end);
                i = end;
            } else if rest.starts_with(b"/*") {
                let (mut depth, mut j) = (0usize, i);
                while j < bytes.len() {
                    if bytes[j..].starts_with(b"/*") {
                        depth += 1;
                        j += 2;
                    } else if bytes[j..].starts_with(b"*/") {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                blank(&mut out, i, j);
                i = j;
            } else if (rest.starts_with(b"r#") || rest.starts_with(b"r\""))
                && (i == 0
                    || !identifier(bytes[i - 1])
                    || (bytes[i - 1] == b'b' && (i == 1 || !identifier(bytes[i - 2]))))
            {
                let hashes = rest[1..].iter().take_while(|&&byte| byte == b'#').count();
                let open = 1 + hashes;
                if rest.get(open) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let mut close = vec![b'"'];
                close.extend(std::iter::repeat_n(b'#', hashes));
                let body = i + open + 1;
                let end = bytes[body..]
                    .windows(close.len())
                    .position(|window| window == close.as_slice())
                    .map_or(bytes.len(), |n| body + n + close.len());
                blank(&mut out, i, end);
                i = end;
            } else if rest[0] == b'"' {
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b'"' {
                    j += if bytes[j] == b'\\' { 2 } else { 1 };
                }
                let end = (j + 1).min(bytes.len());
                blank(&mut out, i, end);
                i = end;
            } else if rest[0] == b'\'' {
                // A char literal closes within a few bytes; a lifetime never
                // closes at all.
                let close = if rest.get(1) == Some(&b'\\') {
                    rest.iter()
                        .skip(2)
                        .position(|&byte| byte == b'\'')
                        .map(|n| n + 2)
                } else {
                    let width = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                    (rest.get(1 + width) == Some(&b'\'')).then_some(1 + width)
                };
                match close {
                    Some(n) if n <= 12 => {
                        blank(&mut out, i, i + n + 1);
                        i += n + 1;
                    }
                    _ => i += 1,
                }
            } else {
                i += 1;
            }
        }
        out
    }

    /// `code` with every `#[cfg(test)]` item — a `mod tests;` line, an inline
    /// test module, a test-only function — blanked as well.
    fn blank_test_items(mut code: Vec<u8>) -> Vec<u8> {
        const ATTRIBUTE: &[u8] = b"#[cfg(test)]";
        let mut i = 0;
        while let Some(found) = code[i..]
            .windows(ATTRIBUTE.len())
            .position(|window| window == ATTRIBUTE)
        {
            let start = i + found;
            let mut j = start + ATTRIBUTE.len();
            let mut depth = 0usize;
            while j < code.len() {
                match code[j] {
                    b';' if depth == 0 => break,
                    b'{' => depth += 1,
                    b'}' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let end = (j + 1).min(code.len());
            for byte in &mut code[start..end] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
            i = end;
        }
        code
    }

    #[test]
    fn test_items_and_literals_are_not_shipping_code() {
        let text = "fn a() { v.sort_by(f) }\n\
                    #[cfg(test)]\nmod tests;\n\
                    // v.sort_by(f)\n\
                    fn b() { let s = \"{ v.sort_by(f)\"; let c = '{'; v.sort_by_key(g) }\n\
                    #[cfg(test)]\nmod inline { fn t() { v.sort_by(f) } }\n\
                    fn c<'a>(v: &'a mut [u8]) { v.sort_unstable_by(f) }\n";
        let shipping = blank_test_items(blank_literals(text));
        let shipping = String::from_utf8(shipping).expect("ASCII stays ASCII");
        let hits: Vec<usize> = shipping
            .lines()
            .enumerate()
            .filter(|(_, line)| STD_COMPARATOR_SORTS.iter().any(|m| line.contains(m)))
            .map(|(number, _)| number + 1)
            .collect();
        assert_eq!(hits, vec![1, 5, 8], "{shipping}");
    }

    #[test]
    fn shipping_code_sorts_through_the_shared_instantiation() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        assert!(
            files.len() > 100,
            "the walk found only {} Rust files",
            files.len()
        );
        let mut offenders = Vec::new();
        for path in files {
            let relative = path.strip_prefix(&src).expect("under src/");
            // The native binaries (`main.rs`, `bin/`) never reach the
            // browser build, so std's sorts cost them nothing.
            if relative.starts_with("bin")
                || relative == Path::new("main.rs")
                || is_test_file(relative)
            {
                continue;
            }
            if relative == Path::new("shared_sort.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source is UTF-8");
            let shipping = blank_test_items(blank_literals(&text));
            let shipping = String::from_utf8_lossy(&shipping);
            for (number, line) in shipping.lines().enumerate() {
                if STD_COMPARATOR_SORTS
                    .iter()
                    .any(|method| line.contains(method))
                {
                    offenders.push(format!("src/{}:{}", relative.display(), number + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "a std comparator sort in shipping code gives the browser build its own copy of \
             the whole sort; call the `_shared` method from `crate::shared_sort::SharedSort` \
             instead:\n{}",
            offenders.join("\n")
        );
    }

    /// Ties carry their original position, so any difference in how equal
    /// elements end up ordered shows.
    fn tied_values() -> Vec<(u8, u32)> {
        let mut state = 0x2545_f491_u32;
        (0..600)
            .map(|position| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                ((state % 7) as u8, position)
            })
            .collect()
    }

    /// Exercises the erased path itself — the one `wasm32` runs — on
    /// whatever target the tests run on. The std side calls exactly the
    /// method each shared one stands in for, so clippy's preferred rewrite
    /// would compare against a different method.
    #[test]
    #[allow(clippy::unnecessary_sort_by)]
    fn the_erased_sorts_order_exactly_as_std_does() {
        use super::{key_order, stable, unstable};
        let values = tied_values();

        let (mut ours, mut std) = (values.clone(), values.clone());
        stable(&mut ours, &mut |a: &(u8, u32), b: &(u8, u32)| b.0.cmp(&a.0));
        std.sort_by(|a, b| b.0.cmp(&a.0));
        assert_eq!(ours, std);

        let (mut ours, mut std) = (values.clone(), values.clone());
        stable(&mut ours, &mut |a: &(u8, u32), b: &(u8, u32)| {
            key_order(a.0, b.0)
        });
        std.sort_by_key(|value| value.0);
        assert_eq!(ours, std);

        let (mut ours, mut std) = (values.clone(), values.clone());
        unstable(&mut ours, &mut |a: &(u8, u32), b: &(u8, u32)| a.0.cmp(&b.0));
        std.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(ours, std);

        let (mut ours, mut std) = (values.clone(), values.clone());
        unstable(&mut ours, &mut |a: &(u8, u32), b: &(u8, u32)| {
            key_order(std::cmp::Reverse(a.0), std::cmp::Reverse(b.0))
        });
        std.sort_unstable_by_key(|value| std::cmp::Reverse(value.0));
        assert_eq!(ours, std);

        // And the trait the call sites use agrees on this target too.
        let (mut ours, mut std) = (values.clone(), values);
        ours.sort_by_key_shared(|value| value.0);
        std.sort_by_key(|value| value.0);
        assert_eq!(ours, std);
    }
}
