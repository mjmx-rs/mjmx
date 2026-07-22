//! Hiding templating syntax from the HTML tokenizer.
//!
//! A rendered mjml document is very often a *template* — Handlebars, EJS,
//! Liquid — that will be filled in later. Those constructs are not markup, but
//! they are made of the same characters as markup, so an HTML tokenizer will
//! happily read `{{ "<div class='a'>" }}` as an open tag and an inliner will
//! then splice a `style` attribute into the middle of a string literal. juice
//! masks its `codeBlocks` before handing the document to cheerio for exactly
//! this reason, and does it *unconditionally* — EJS and Handlebars are masked on
//! every call, whether or not `juicePreserveTags` was passed. mjmx matches that.
//!
//! ## Equal-length masking
//!
//! juice substitutes `JUICE_CODE_BLOCK_<n>_`, a placeholder of a different
//! length, and has to run a textual restore pass over the serialized output.
//! mjmx cannot: the whole inliner is splice-based, addressing the document by
//! byte offset, and a length-changing mask would invalidate every offset.
//!
//! So the mask here is **byte-length preserving** — each block is overwritten
//! with a run of an inert ASCII byte. The masked copy is what gets tokenized and
//! what the element tree borrows from; the edits it produces are then spliced
//! into the *original*, which restores every masked region for free and needs no
//! decode pass at all. The one place a masked byte could still leak into the
//! output is an existing `style` attribute's value, which
//! [`apply`](crate::apply) therefore reads back through its span in the original.

use crate::options::PreserveTag;

/// The byte a masked region is filled with. Inert in text, in an attribute
/// value, and inside a tag: it cannot start a tag, close one, open a comment or
/// terminate a quoted value.
const MASK: u8 = b'x';

/// Overwrite every code block in `html` with an equal-length inert run.
///
/// Returns `None` when no tag matched, so the common case allocates nothing.
/// Tags are applied in order, and a region already masked cannot match a later
/// tag — its delimiters are gone — which is how juice's sequential
/// `String.replace` per key behaves too.
pub(crate) fn mask(html: &str, tags: &[PreserveTag]) -> Option<String> {
    // Cheap rejection: masking allocates a copy of the document, so do not pay
    // for it unless some opening delimiter is actually present.
    if !tags
        .iter()
        .any(|tag| !tag.start.is_empty() && !tag.end.is_empty() && html.contains(&tag.start))
    {
        return None;
    }

    let mut bytes = html.as_bytes().to_vec();
    let mut masked = false;
    for tag in tags {
        if tag.start.is_empty() || tag.end.is_empty() {
            continue;
        }
        let mut from = 0;
        while let Some(start) = find(&bytes[from..], tag.start.as_bytes()).map(|at| from + at) {
            let after = start + tag.start.len();
            // Non-greedy, exactly like juice's `start([\S\s]*?)end`: the block
            // runs to the *nearest* closing delimiter. An unterminated opener is
            // not a block at all, and stops this tag's scan.
            let Some(end) = find(&bytes[after..], tag.end.as_bytes()).map(|at| after + at) else {
                break;
            };
            let end = end + tag.end.len();
            bytes[start..end].fill(MASK);
            masked = true;
            from = end;
        }
    }

    masked.then(|| {
        // Every replaced region began and ended on a character boundary (the
        // delimiters are matched as whole byte strings starting at one), and the
        // replacement is ASCII, so the result is still UTF-8.
        String::from_utf8(bytes).expect("masking replaces whole characters with ASCII")
    })
}

/// The first occurrence of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Vec<PreserveTag> {
        PreserveTag::defaults()
    }

    #[test]
    fn a_document_with_no_templating_is_not_copied() {
        assert_eq!(mask("<p>plain</p>", &defaults()), None);
    }

    #[test]
    fn handlebars_and_ejs_are_masked_by_default() {
        let masked = mask("<p>{{ a }}</p><b><% x %></b>", &defaults()).unwrap();
        assert_eq!(masked, "<p>xxxxxxx</p><b>xxxxxxx</b>");
    }

    #[test]
    fn the_mask_preserves_byte_length() {
        let html = "<p title=\"{{ é }}\">{{ \"<div>\" }}</p>";
        let masked = mask(html, &defaults()).unwrap();
        assert_eq!(masked.len(), html.len());
        assert!(!masked.contains("<div>"), "{masked}");
        assert!(masked.starts_with("<p title=\"x"), "{masked}");
    }

    #[test]
    fn matching_is_non_greedy() {
        let masked = mask("{{a}}keep{{b}}", &defaults()).unwrap();
        assert_eq!(masked, "xxxxxkeepxxxxx");
    }

    #[test]
    fn an_unterminated_opener_is_left_alone() {
        assert_eq!(mask("<p>{{ oops</p>", &defaults()), None);
    }

    #[test]
    fn a_registered_tag_is_masked_too() {
        let tags = vec![PreserveTag::new("myTag", "<#", "</#")];
        let masked = mask("a<# raw </# b", &tags).unwrap();
        assert_eq!(masked, "axxxxxxxxxx b");
    }

    #[test]
    fn a_masked_region_cannot_match_a_later_tag() {
        // `<% {{ %>` is one EJS block; the Handlebars pass must not see the `{{`
        // inside it, and must not run off looking for a `}}`.
        let masked = mask("<% {{ %>{{a}}", &defaults()).unwrap();
        assert_eq!(masked, "xxxxxxxxxxxxx");
    }
}
