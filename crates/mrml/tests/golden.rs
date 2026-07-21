//! End-to-end golden tests: each `fixtures/<name>.mjml` must render to
//! `fixtures/<name>.html` (the vendored mjml 4.18.0 reference) under
//! whitespace-normalized comparison.

/// Absolute path to a fixture file under `fixtures/`.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/",
            $name
        ))
    };
}

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

/// Render `mjml` and assert it matches `golden` under [`normalize`], surfacing the
/// first divergence to make failures debuggable.
fn assert_golden(name: &str, mjml: &str, golden: &str) {
    let rendered =
        mrml::render_default(mjml).unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));

    let got = normalize(&rendered);
    let want = normalize(golden);

    if got != want {
        let first_diff = got
            .char_indices()
            .zip(want.char_indices())
            .find(|((_, a), (_, b))| a != b)
            .map(|((gi, _), _)| gi)
            .unwrap_or(got.len().min(want.len()));
        let ctx_start = first_diff.saturating_sub(80);
        panic!(
            "{name}: rendered HTML does not match golden reference.\n\
             first divergence near byte {first_diff}:\n\
             GOT : ...{}\n\
             WANT: ...{}\n",
            &got[ctx_start..(first_diff + 120).min(got.len())],
            &want[ctx_start..(first_diff + 120).min(want.len())],
        );
    }

    assert_eq!(
        got, want,
        "{name}: normalized HTML must match the mjml reference"
    );
}

#[test]
fn slice_matches_mjml_reference() {
    assert_golden("slice", fixture!("slice.mjml"), fixture!("slice.html"));
}

#[test]
fn attrs_all_matches_mjml_reference() {
    assert_golden(
        "attrs_all",
        fixture!("attrs_all.mjml"),
        fixture!("attrs_all.html"),
    );
}

#[test]
fn attrs_class_matches_mjml_reference() {
    assert_golden(
        "attrs_class",
        fixture!("attrs_class.mjml"),
        fixture!("attrs_class.html"),
    );
}

#[test]
fn head_breakpoint_matches_mjml_reference() {
    assert_golden(
        "head_breakpoint",
        fixture!("head_breakpoint.mjml"),
        fixture!("head_breakpoint.html"),
    );
}

#[test]
fn head_title_preview_matches_mjml_reference() {
    assert_golden(
        "head_title_preview",
        fixture!("head_title_preview.mjml"),
        fixture!("head_title_preview.html"),
    );
}

#[test]
fn head_font_matches_mjml_reference() {
    assert_golden(
        "head_font",
        fixture!("head_font.mjml"),
        fixture!("head_font.html"),
    );
}

#[test]
fn text_attrs_matches_mjml_reference() {
    assert_golden(
        "text_attrs",
        fixture!("text_attrs.mjml"),
        fixture!("text_attrs.html"),
    );
}

#[test]
fn img_matches_mjml_reference() {
    assert_golden("img", fixture!("img.mjml"), fixture!("img.html"));
}

#[test]
fn image_attrs_matches_mjml_reference() {
    assert_golden(
        "image_attrs",
        fixture!("image_attrs.mjml"),
        fixture!("image_attrs.html"),
    );
}

#[test]
fn img_link_matches_mjml_reference() {
    assert_golden(
        "img_link",
        fixture!("img_link.mjml"),
        fixture!("img_link.html"),
    );
}

#[test]
fn divider_matches_mjml_reference() {
    assert_golden(
        "divider",
        fixture!("divider.mjml"),
        fixture!("divider.html"),
    );
}

#[test]
fn divider_attrs_matches_mjml_reference() {
    assert_golden(
        "divider_attrs",
        fixture!("divider_attrs.mjml"),
        fixture!("divider_attrs.html"),
    );
}

#[test]
fn spacer_matches_mjml_reference() {
    assert_golden("spacer", fixture!("spacer.mjml"), fixture!("spacer.html"));
}

#[test]
fn spacer_attrs_matches_mjml_reference() {
    assert_golden(
        "spacer_attrs",
        fixture!("spacer_attrs.mjml"),
        fixture!("spacer_attrs.html"),
    );
}

#[test]
fn button_matches_mjml_reference() {
    assert_golden("button", fixture!("button.mjml"), fixture!("button.html"));
}

#[test]
fn button_attrs_matches_mjml_reference() {
    assert_golden(
        "button_attrs",
        fixture!("button_attrs.mjml"),
        fixture!("button_attrs.html"),
    );
}

#[test]
fn section_attrs_matches_mjml_reference() {
    assert_golden(
        "section_attrs",
        fixture!("section_attrs.mjml"),
        fixture!("section_attrs.html"),
    );
}

#[test]
fn twocol_matches_mjml_reference() {
    assert_golden("twocol", fixture!("twocol.mjml"), fixture!("twocol.html"));
}

#[test]
fn column_attrs_matches_mjml_reference() {
    assert_golden(
        "column_attrs",
        fixture!("column_attrs.mjml"),
        fixture!("column_attrs.html"),
    );
}

#[test]
fn raw_matches_mjml_reference() {
    assert_golden("raw", fixture!("raw.mjml"), fixture!("raw.html"));
}

#[test]
fn group_matches_mjml_reference() {
    assert_golden("group", fixture!("group.mjml"), fixture!("group.html"));
}

#[test]
fn group_attrs_matches_mjml_reference() {
    assert_golden(
        "group_attrs",
        fixture!("group_attrs.mjml"),
        fixture!("group_attrs.html"),
    );
}

#[test]
fn wrapper_matches_mjml_reference() {
    assert_golden(
        "wrapper",
        fixture!("wrapper.mjml"),
        fixture!("wrapper.html"),
    );
}

#[test]
fn wrapper_full_width_matches_mjml_reference() {
    assert_golden(
        "wrapper_full_width",
        fixture!("wrapper_full_width.mjml"),
        fixture!("wrapper_full_width.html"),
    );
}

#[test]
fn wrapper_attrs_matches_mjml_reference() {
    assert_golden(
        "wrapper_attrs",
        fixture!("wrapper_attrs.mjml"),
        fixture!("wrapper_attrs.html"),
    );
}

#[test]
fn table_matches_mjml_reference() {
    assert_golden("table", fixture!("table.mjml"), fixture!("table.html"));
}

#[test]
fn table_attrs_matches_mjml_reference() {
    assert_golden(
        "table_attrs",
        fixture!("table_attrs.mjml"),
        fixture!("table_attrs.html"),
    );
}

#[test]
fn social_matches_mjml_reference() {
    assert_golden("social", fixture!("social.mjml"), fixture!("social.html"));
}

#[test]
fn social_attrs_matches_mjml_reference() {
    assert_golden(
        "social_attrs",
        fixture!("social_attrs.mjml"),
        fixture!("social_attrs.html"),
    );
}

#[test]
fn social_vertical_matches_mjml_reference() {
    assert_golden(
        "social_vertical",
        fixture!("social_vertical.mjml"),
        fixture!("social_vertical.html"),
    );
}

#[test]
fn navbar_matches_mjml_reference() {
    assert_golden("navbar", fixture!("navbar.mjml"), fixture!("navbar.html"));
}

#[test]
fn navbar_hamburger_matches_mjml_reference() {
    // The vendored golden's random 16-hex checkbox id has been pinned to the
    // deterministic value our renderer emits (see RenderContext::next_hamburger_id);
    // every other byte is mjml 4.18.0's own output.
    assert_golden(
        "navbar_hamburger",
        fixture!("navbar_hamburger.mjml"),
        fixture!("navbar_hamburger.html"),
    );
}

#[test]
fn navbar_attrs_matches_mjml_reference() {
    assert_golden(
        "navbar_attrs",
        fixture!("navbar_attrs.mjml"),
        fixture!("navbar_attrs.html"),
    );
}

#[test]
fn hero_matches_mjml_reference() {
    assert_golden("hero", fixture!("hero.mjml"), fixture!("hero.html"));
}

#[test]
fn hero_attrs_matches_mjml_reference() {
    assert_golden(
        "hero_attrs",
        fixture!("hero_attrs.mjml"),
        fixture!("hero_attrs.html"),
    );
}

#[test]
fn carousel_matches_mjml_reference() {
    // mjml stamps the carousel radio group with a per-carousel random 16-hex token
    // (`genRandomHexString(16)`); the vendored golden's token is pinned to the
    // deterministic value our renderer emits (see RenderContext::next_carousel_id).
    assert_golden(
        "carousel",
        fixture!("carousel.mjml"),
        fixture!("carousel.html"),
    );
}

#[test]
fn accordion_matches_mjml_reference() {
    assert_golden(
        "accordion",
        fixture!("accordion.mjml"),
        fixture!("accordion.html"),
    );
}

#[test]
fn accordion_attrs_matches_mjml_reference() {
    assert_golden(
        "accordion_attrs",
        fixture!("accordion_attrs.mjml"),
        fixture!("accordion_attrs.html"),
    );
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
