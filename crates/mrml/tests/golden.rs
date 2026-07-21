//! End-to-end golden test: `fixtures/slice.mjml` must render to `fixtures/slice.html`
//! (the vendored mjml 4.18.0 reference) under whitespace-normalized comparison.

const MJML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/slice.mjml"
));
const GOLDEN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/slice.html"
));

/// Normalize HTML for semantic comparison.
///
/// This is deliberately tolerant of pure indentation / newline differences but
/// strict on everything else (tags, attributes, text, CSS tokens): a whitespace run
/// that sits directly between a `>` and a `<` is *insignificant* inter-tag layout and
/// is dropped; every other whitespace run collapses to a single space. Text content,
/// attribute values, CSS tokens and their separators are all preserved, so a wrong
/// tag, attribute, or text still fails the comparison.
fn normalize(html: &str) -> String {
    let chars: Vec<char> = html.chars().collect();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let prev = out.chars().last();
            let next = chars.get(i).copied();
            // Drop whitespace that is purely between two tags (`>` ... `<`); every
            // other run collapses to a single space.
            if prev == Some('>') && next == Some('<') {
                // insignificant inter-tag whitespace — drop it
            } else {
                out.push(' ');
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out.trim().to_string()
}

#[test]
fn slice_matches_mjml_reference() {
    let rendered = mrml::render_default(MJML).expect("slice.mjml should render without errors");

    let got = normalize(&rendered);
    let want = normalize(GOLDEN);

    if got != want {
        // Surface the first divergence to make failures debuggable.
        let first_diff = got
            .char_indices()
            .zip(want.char_indices())
            .find(|((_, a), (_, b))| a != b)
            .map(|((gi, _), _)| gi)
            .unwrap_or(got.len().min(want.len()));
        let ctx_start = first_diff.saturating_sub(80);
        panic!(
            "rendered HTML does not match golden reference.\n\
             first divergence near byte {first_diff}:\n\
             GOT : ...{}\n\
             WANT: ...{}\n",
            &got[ctx_start..(first_diff + 120).min(got.len())],
            &want[ctx_start..(first_diff + 120).min(want.len())],
        );
    }

    assert_eq!(got, want, "normalized HTML must match the mjml reference");
}

#[test]
fn normalize_is_strict_about_content() {
    // The comparator must NOT be so tolerant that wrong content passes.
    assert_ne!(
        normalize("<div>Hello world</div>"),
        normalize("<div>Goodbye world</div>"),
        "text differences must be detected"
    );
    assert_ne!(
        normalize("<td style=\"padding:10px 25px;\">x</td>"),
        normalize("<td style=\"padding:20px 25px;\">x</td>"),
        "attribute differences must be detected"
    );
    // ...but tolerant of indentation / newlines.
    assert_eq!(
        normalize("<a>\n  <b>x</b>\n</a>"),
        normalize("<a><b>x</b></a>"),
        "pure inter-tag whitespace must be ignored"
    );
}
