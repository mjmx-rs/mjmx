//! Self-tests for the semantic HTML comparator: it must be tolerant of
//! insignificant differences (order, whitespace) yet strict about anything that
//! changes meaning (tags, text, attribute/style/class values, comments).

use mjmx_html_compare::compare;

fn assert_eq_html(expected: &str, actual: &str) {
    if let Err(m) = compare(expected, actual) {
        panic!("expected documents to be equivalent, but:\n{m}");
    }
}

fn assert_ne_html(expected: &str, actual: &str) {
    assert!(
        compare(expected, actual).is_err(),
        "expected documents to differ, but they compared equal"
    );
}

// --- tolerant of insignificant differences -----------------------------------

#[test]
fn identical_documents_are_equal() {
    let doc = "<!doctype html><html><body><p>Hi</p></body></html>";
    assert_eq_html(doc, doc);
}

#[test]
fn attribute_order_is_ignored() {
    assert_eq_html(
        r#"<img src="a.png" width="300" alt="cat">"#,
        r#"<img alt="cat" width="300" src="a.png">"#,
    );
}

#[test]
fn style_declaration_order_is_ignored() {
    assert_eq_html(
        r#"<div style="width:100%;color:#000;font-size:13px;">x</div>"#,
        r#"<div style="font-size:13px;width:100%;color:#000;">x</div>"#,
    );
}

#[test]
fn style_declaration_spacing_is_ignored() {
    assert_eq_html(
        r#"<div style="width:100%; color: #000;">x</div>"#,
        r#"<div style="color:#000;width:100%">x</div>"#,
    );
}

#[test]
fn class_token_order_is_ignored() {
    assert_eq_html(
        r#"<div class="a b c">x</div>"#,
        r#"<div class="c a b">x</div>"#,
    );
}

#[test]
fn insignificant_whitespace_is_ignored() {
    assert_eq_html("<a>\n  <b>x</b>\n</a>", "<a><b>x</b></a>");
}

#[test]
fn indentation_and_newlines_in_text_are_collapsed() {
    assert_eq_html("<p>Hello    world</p>", "<p>Hello\n  world</p>");
}

#[test]
fn doctype_case_is_ignored() {
    assert_eq_html(
        "<!DOCTYPE HTML><html></html>",
        "<!doctype html><html></html>",
    );
}

#[test]
fn tag_case_is_ignored() {
    assert_eq_html("<DIV>x</DIV>", "<div>x</div>");
}

#[test]
fn void_and_self_closing_are_equivalent() {
    assert_eq_html(r#"<img src="a.png"><br>"#, r#"<img src="a.png" /><br/>"#);
}

#[test]
fn mso_conditional_comment_matches() {
    let doc = "<!--[if mso | IE]><table align=\"center\" width=\"600\"><tr><td><![endif]-->";
    assert_eq_html(doc, doc);
    // Same comment, reformatted whitespace inside stays equal.
    assert_eq_html(
        "<!--[if mso | IE]>  <table>  <![endif]-->",
        "<!--[if mso | IE]> <table> <![endif]-->",
    );
}

#[test]
fn raw_text_style_block_compares_by_content() {
    assert_eq_html(
        "<style type=\"text/css\">\n  .a { width: 100%; }\n</style>",
        "<style type=\"text/css\">.a { width: 100%; }</style>",
    );
}

// --- strict about meaningful differences -------------------------------------

#[test]
fn differing_text_is_a_mismatch() {
    assert_ne_html("<div>Hello world</div>", "<div>Goodbye world</div>");
}

#[test]
fn missing_element_is_a_mismatch() {
    assert_ne_html("<div><p>x</p><span>y</span></div>", "<div><p>x</p></div>");
}

#[test]
fn extra_element_is_a_mismatch() {
    assert_ne_html("<div><p>x</p></div>", "<div><p>x</p><span>y</span></div>");
}

#[test]
fn wrong_tag_is_a_mismatch() {
    assert_ne_html("<div>x</div>", "<span>x</span>");
}

#[test]
fn wrong_align_value_is_a_mismatch() {
    // Regression guard mirroring a real bug: a flipped alignment must be caught.
    assert_ne_html(r#"<td align="left">x</td>"#, r#"<td align="right">x</td>"#);
}

#[test]
fn wrong_width_style_declaration_is_a_mismatch() {
    // Regression guard: width:100% vs width:50% on a div must be caught.
    assert_ne_html(
        r#"<div style="width:100%;">x</div>"#,
        r#"<div style="width:50%;">x</div>"#,
    );
}

#[test]
fn extra_style_declaration_is_a_mismatch() {
    assert_ne_html(
        r#"<div style="width:100%;">x</div>"#,
        r#"<div style="width:100%;color:red;">x</div>"#,
    );
}

#[test]
fn removed_style_declaration_is_a_mismatch() {
    assert_ne_html(
        r#"<div style="width:100%;color:red;">x</div>"#,
        r#"<div style="width:100%;">x</div>"#,
    );
}

#[test]
fn extra_attribute_is_a_mismatch() {
    assert_ne_html(r#"<td>x</td>"#, r#"<td valign="top">x</td>"#);
}

#[test]
fn missing_attribute_is_a_mismatch() {
    assert_ne_html(r#"<td valign="top">x</td>"#, r#"<td>x</td>"#);
}

#[test]
fn differing_attribute_value_is_a_mismatch() {
    assert_ne_html(r#"<td width="600">x</td>"#, r#"<td width="300">x</td>"#);
}

#[test]
fn differing_class_token_is_a_mismatch() {
    assert_ne_html(r#"<div class="a b">x</div>"#, r#"<div class="a c">x</div>"#);
}

#[test]
fn differing_mso_comment_is_a_mismatch() {
    assert_ne_html(
        "<!--[if mso | IE]><table><![endif]-->",
        "<!--[if mso | IE]><span><![endif]-->",
    );
}

#[test]
fn differing_raw_style_content_is_a_mismatch() {
    assert_ne_html(
        "<style>.a { width: 100%; }</style>",
        "<style>.a { width: 50%; }</style>",
    );
}

#[test]
fn mismatch_message_points_at_the_divergence() {
    let err = compare(
        "<div><p>Hello world</p></div>",
        "<div><p>Goodbye world</p></div>",
    )
    .expect_err("should differ");
    let msg = err.to_string();
    assert!(msg.contains("text differs"), "message: {msg}");
    assert!(msg.contains("div > p"), "path breadcrumb missing: {msg}");
}
