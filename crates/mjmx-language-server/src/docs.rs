//! A built-in documentation table for MJML components and attributes.
//!
//! This is the single place the server keeps human-facing prose, used by both
//! `textDocument/hover` and the `documentation` field of completion items.
//!
//! **It is deliberately non-exhaustive.** MJML 4 exposes a large attribute surface
//! (much of it shared boilerplate like `css-class` or the `padding-*` family); this
//! table covers each component's summary plus its most commonly used attributes.
//! Anything missing simply hovers/completes without prose — never with a wrong one.
//! Extending it is purely additive: add a row, no other code changes.

use mjmx_syntax::MjmlTag;

/// Attributes every visible body component accepts, documented once.
const COMMON: &[(&str, &str)] = &[
    (
        "css-class",
        "Extra CSS class name applied to the outer HTML element.",
    ),
    ("padding", "Shorthand padding, e.g. `10px 25px`."),
    ("padding-top", "Top padding."),
    ("padding-bottom", "Bottom padding."),
    ("padding-left", "Left padding."),
    ("padding-right", "Right padding."),
];

/// A one-line summary of what `tag` is.
#[must_use]
pub fn tag_summary(tag: MjmlTag) -> Option<&'static str> {
    Some(match tag {
        MjmlTag::Mjml => "Root element of an MJML document. Contains `<mj-head>` and `<mj-body>`.",
        MjmlTag::MjHead => "Document head: metadata, styles, fonts and global attribute defaults.",
        MjmlTag::MjBody => "Document body: the renderable content of the email.",
        MjmlTag::MjAttributes => {
            "Declares default attributes per component or `mj-class`, applied document-wide."
        }
        MjmlTag::MjAll => {
            "An `<mj-attributes>` row: sets a default attribute on *every* component."
        }
        MjmlTag::MjClass => {
            "An `<mj-attributes>` row: a named attribute set, applied with `mj-class=\"name\"`."
        }
        MjmlTag::MjBreakpoint => "Sets the width below which the layout switches to mobile.",
        MjmlTag::MjFont => "Imports a web font by `name` and `href`.",
        MjmlTag::MjPreview => "Preview text shown by mail clients next to the subject line.",
        MjmlTag::MjStyle => {
            "Raw CSS injected into the rendered `<style>` tag (`inline` to inline it)."
        }
        MjmlTag::MjTitle => "The document `<title>`.",
        MjmlTag::MjHtmlAttributes => {
            "Adds arbitrary HTML attributes to elements matched by a selector."
        }
        MjmlTag::MjSelector => {
            "Selects elements by CSS `path` for `<mj-html-attributes>` to annotate."
        }
        MjmlTag::MjHtmlAttribute => {
            "One attribute (`name`) to add to the elements its `<mj-selector>` matches."
        }
        MjmlTag::MjSection => "A horizontal row of the layout. Contains `mj-column` or `mj-group`.",
        MjmlTag::MjColumn => "A vertical container inside a section; holds content components.",
        MjmlTag::MjGroup => "Keeps its columns side by side on mobile instead of stacking them.",
        MjmlTag::MjWrapper => "Wraps several sections so they share a background and border.",
        MjmlTag::MjText => "A block of text. The body is raw HTML and is passed through verbatim.",
        MjmlTag::MjButton => "A clickable, bulletproof button. The body is the label (raw HTML).",
        MjmlTag::MjImage => "An image. `src` is required.",
        MjmlTag::MjDivider => "A horizontal rule.",
        MjmlTag::MjSpacer => "Vertical blank space of a given `height`.",
        MjmlTag::MjRaw => "Raw HTML passed straight through, untouched by MJML.",
        MjmlTag::MjTable => "A `<table>`; the body is raw HTML table markup.",
        MjmlTag::MjSocial => "A row of social-network icons. Contains `mj-social-element`.",
        MjmlTag::MjSocialElement => "One social icon/link inside `mj-social`.",
        MjmlTag::MjNavbar => {
            "A navigation bar with a hamburger fallback. Contains `mj-navbar-link`."
        }
        MjmlTag::MjNavbarLink => "One link inside `mj-navbar`.",
        MjmlTag::MjHero => {
            "A hero banner: a full-width image or colour background with content on top."
        }
        MjmlTag::MjCarousel => "An image carousel. Contains `mj-carousel-image`.",
        MjmlTag::MjCarouselImage => "One slide of an `mj-carousel`.",
        MjmlTag::MjAccordion => "A collapsible list. Contains `mj-accordion-element`.",
        MjmlTag::MjAccordionElement => {
            "One accordion entry: an `mj-accordion-title` plus `mj-accordion-text`."
        }
        MjmlTag::MjAccordionTitle => "The always-visible header of an accordion entry.",
        MjmlTag::MjAccordionText => "The collapsible body of an accordion entry.",
        MjmlTag::MjInclude => "Includes another MJML file at this position (`path`).",
        MjmlTag::Comment | MjmlTag::Text | MjmlTag::Unknown => return None,
    })
}

/// The documented attributes of `tag`, as `(name, description)` pairs.
///
/// Component-specific attributes come first, then the shared layout attributes for
/// components that accept them.
#[must_use]
pub fn tag_attributes(tag: MjmlTag) -> Vec<(&'static str, &'static str)> {
    let mut attributes: Vec<(&'static str, &'static str)> = specific(tag).to_vec();
    if accepts_common(tag) {
        for (name, description) in COMMON {
            if !attributes.iter().any(|(existing, _)| existing == name) {
                attributes.push((name, description));
            }
        }
    }
    attributes
}

/// The description of `attribute` on `tag`, if documented.
#[must_use]
pub fn attribute_doc(tag: MjmlTag, attribute: &str) -> Option<&'static str> {
    tag_attributes(tag)
        .into_iter()
        .find(|(name, _)| *name == attribute)
        .map(|(_, description)| description)
}

/// Whether `tag` is a visible body component that accepts the [`COMMON`] attributes.
fn accepts_common(tag: MjmlTag) -> bool {
    matches!(
        tag,
        MjmlTag::MjSection
            | MjmlTag::MjColumn
            | MjmlTag::MjGroup
            | MjmlTag::MjWrapper
            | MjmlTag::MjText
            | MjmlTag::MjButton
            | MjmlTag::MjImage
            | MjmlTag::MjDivider
            | MjmlTag::MjSpacer
            | MjmlTag::MjTable
            | MjmlTag::MjSocial
            | MjmlTag::MjSocialElement
            | MjmlTag::MjNavbar
            | MjmlTag::MjHero
            | MjmlTag::MjCarousel
            | MjmlTag::MjAccordion
    )
}

/// The attributes unique to `tag`.
#[allow(clippy::too_many_lines)]
fn specific(tag: MjmlTag) -> &'static [(&'static str, &'static str)] {
    match tag {
        MjmlTag::Mjml => &[(
            "lang",
            "Document language, emitted as the `lang` attribute on `<html>`.",
        )],
        MjmlTag::MjBody => &[("width", "Width of the email body. Default `600px`.")],
        MjmlTag::MjBreakpoint => &[(
            "width",
            "Viewport width below which the mobile layout applies, e.g. `480px`.",
        )],
        MjmlTag::MjFont => &[
            ("name", "Font family name to register."),
            (
                "href",
                "URL of the font stylesheet (e.g. a Google Fonts link).",
            ),
        ],
        MjmlTag::MjStyle => &[(
            "inline",
            "Set to `inline` to inline this CSS onto the elements it matches.",
        )],
        MjmlTag::MjHtmlAttributes => &[(
            "path",
            "CSS selector whose matched elements receive the declared attributes.",
        )],
        MjmlTag::MjAttributes => &[(
            "name",
            "On a child `mj-class`, the class name referenced via `mj-class=\"…\"`.",
        )],
        MjmlTag::MjSection => &[
            ("background-color", "Section background colour."),
            ("background-url", "Background image URL."),
            ("background-repeat", "`repeat` (default) or `no-repeat`."),
            ("background-size", "Background image size, e.g. `cover`."),
            ("border", "Shorthand border, e.g. `1px solid #ccc`."),
            ("border-radius", "Corner radius."),
            ("direction", "`ltr` (default) or `rtl` — column order."),
            (
                "full-width",
                "Set to `full-width` to span the whole viewport.",
            ),
            ("text-align", "Horizontal alignment of the section content."),
        ],
        MjmlTag::MjColumn => &[
            (
                "width",
                "Column width in `px` or `%`. Defaults to an even split.",
            ),
            ("background-color", "Column background colour."),
            ("border", "Shorthand border."),
            ("border-radius", "Corner radius."),
            ("vertical-align", "`top` (default), `middle` or `bottom`."),
            (
                "inner-background-color",
                "Background colour of the inner table.",
            ),
        ],
        MjmlTag::MjGroup => &[
            ("width", "Group width in `px` or `%`."),
            ("background-color", "Group background colour."),
            ("vertical-align", "`top` (default), `middle` or `bottom`."),
            ("direction", "`ltr` (default) or `rtl`."),
        ],
        MjmlTag::MjWrapper => &[
            ("background-color", "Wrapper background colour."),
            ("background-url", "Background image URL."),
            ("border", "Shorthand border."),
            (
                "full-width",
                "Set to `full-width` to span the whole viewport.",
            ),
        ],
        MjmlTag::MjText => &[
            ("color", "Text colour. Default `#000000`."),
            (
                "font-family",
                "Font stack. Default `Ubuntu, Helvetica, Arial, sans-serif`.",
            ),
            ("font-size", "Font size. Default `13px`."),
            ("font-style", "e.g. `italic`."),
            ("font-weight", "e.g. `bold` or `400`."),
            ("line-height", "Line height. Default `1`."),
            ("letter-spacing", "Letter spacing, e.g. `1px`."),
            ("align", "`left` (default), `center`, `right` or `justify`."),
            ("text-decoration", "e.g. `underline`."),
            ("text-transform", "e.g. `uppercase`."),
            ("height", "Fixed height of the text block."),
            (
                "container-background-color",
                "Background colour of the enclosing cell.",
            ),
        ],
        MjmlTag::MjButton => &[
            ("href", "Destination URL."),
            ("target", "Link target. Default `_blank`."),
            ("rel", "Link `rel` attribute."),
            ("background-color", "Button background. Default `#414141`."),
            ("color", "Label colour. Default `#ffffff`."),
            ("border-radius", "Corner radius. Default `3px`."),
            ("border", "Shorthand border. Default `none`."),
            ("font-family", "Font stack."),
            ("font-size", "Font size. Default `13px`."),
            ("font-weight", "Font weight."),
            (
                "inner-padding",
                "Padding inside the button. Default `10px 25px`.",
            ),
            ("width", "Fixed button width."),
            ("height", "Fixed button height."),
            ("align", "Horizontal alignment. Default `center`."),
            ("text-align", "Alignment of the label text."),
            ("text-decoration", "Label text decoration. Default `none`."),
            ("vertical-align", "`middle` (default), `top` or `bottom`."),
            (
                "container-background-color",
                "Background colour of the enclosing cell.",
            ),
        ],
        MjmlTag::MjImage => &[
            ("src", "**Required.** Image URL."),
            ("srcset", "Responsive `srcset` list."),
            ("alt", "Alternative text. Default empty."),
            ("href", "Wrap the image in a link to this URL."),
            ("target", "Link target. Default `_blank`."),
            ("rel", "Link `rel` attribute."),
            ("title", "Image `title` attribute."),
            ("width", "Image width; capped by the column width."),
            ("height", "Image height. Default `auto`."),
            ("align", "`center` (default), `left` or `right`."),
            ("border", "Shorthand border. Default `none`."),
            ("border-radius", "Corner radius."),
            (
                "fluid-on-mobile",
                "Set to `true` to force full width on mobile.",
            ),
            (
                "container-background-color",
                "Background colour of the enclosing cell.",
            ),
        ],
        MjmlTag::MjDivider => &[
            ("border-color", "Rule colour. Default `#000000`."),
            ("border-style", "Rule style. Default `solid`."),
            ("border-width", "Rule thickness. Default `4px`."),
            ("width", "Rule width. Default `100%`."),
            ("align", "`center` (default), `left` or `right`."),
            (
                "container-background-color",
                "Background colour of the enclosing cell.",
            ),
        ],
        MjmlTag::MjSpacer => &[
            ("height", "Height of the blank space. Default `20px`."),
            (
                "container-background-color",
                "Background colour of the enclosing cell.",
            ),
        ],
        MjmlTag::MjRaw => &[(
            "position",
            "Set to `file-start` on a raw block outside `mj-body` to emit it before the doctype.",
        )],
        MjmlTag::MjTable => &[
            ("align", "`left` (default), `center` or `right`."),
            ("border", "Shorthand border. Default `none`."),
            ("cellpadding", "HTML `cellpadding`."),
            ("cellspacing", "HTML `cellspacing`."),
            ("color", "Text colour. Default `#000000`."),
            ("font-family", "Font stack."),
            ("font-size", "Font size. Default `13px`."),
            ("line-height", "Line height. Default `22px`."),
            ("table-layout", "`auto` (default) or `fixed`."),
            ("width", "Table width. Default `100%`."),
        ],
        MjmlTag::MjSocial => &[
            ("mode", "`horizontal` (default) or `vertical`."),
            ("icon-size", "Icon size. Default `20px`."),
            ("icon-height", "Icon height; defaults to `icon-size`."),
            ("border-radius", "Icon corner radius. Default `3px`."),
            ("color", "Label colour."),
            ("font-family", "Label font stack."),
            ("font-size", "Label font size. Default `13px`."),
            ("align", "`center` (default), `left` or `right`."),
            ("inner-padding", "Padding inside each element."),
            ("text-decoration", "Label text decoration."),
            (
                "container-background-color",
                "Background colour of the enclosing cell.",
            ),
        ],
        MjmlTag::MjSocialElement => &[
            (
                "name",
                "Known network (`facebook`, `twitter`, `github`, …); supplies the icon and colour.",
            ),
            (
                "src",
                "Custom icon URL, when `name` is not one of the built-ins.",
            ),
            (
                "href",
                "Destination URL. `[[SHORT_PERMALINK]]` placeholders are substituted.",
            ),
            ("alt", "Icon alternative text."),
            ("background-color", "Icon background colour."),
            ("icon-size", "Icon size."),
            ("border-radius", "Icon corner radius."),
            ("target", "Link target. Default `_blank`."),
        ],
        MjmlTag::MjNavbar => &[
            ("align", "`center` (default), `left` or `right`."),
            ("hamburger", "Set to `hamburger` to enable the mobile menu."),
            ("ico-color", "Hamburger icon colour."),
            ("ico-font-size", "Hamburger icon size."),
            ("base-url", "Prefix prepended to every child link's `href`."),
        ],
        MjmlTag::MjNavbarLink => &[
            ("href", "Destination URL."),
            ("target", "Link target. Default `_blank`."),
            ("rel", "Link `rel` attribute."),
            ("color", "Label colour. Default `#000000`."),
            ("font-family", "Font stack."),
            ("font-size", "Font size. Default `13px`."),
            ("font-weight", "Font weight."),
            ("letter-spacing", "Letter spacing."),
            ("line-height", "Line height. Default `22px`."),
            ("text-decoration", "Text decoration. Default `none`."),
            ("text-transform", "e.g. `uppercase`."),
            ("padding", "Padding around the link."),
        ],
        MjmlTag::MjHero => &[
            ("mode", "`fluid-height` (default) or `fixed-height`."),
            ("height", "Hero height when `mode=\"fixed-height\"`."),
            ("background-url", "Background image URL."),
            ("background-color", "Background colour. Default `#ffffff`."),
            ("background-width", "Background image width."),
            ("background-height", "Background image height."),
            (
                "background-position",
                "Background position. Default `center center`.",
            ),
            ("vertical-align", "`top` (default), `middle` or `bottom`."),
            ("width", "Hero width."),
        ],
        MjmlTag::MjCarousel => &[
            ("align", "`center` (default), `left` or `right`."),
            ("thumbnails", "`visible` (default) or `hidden`."),
            ("tb-width", "Thumbnail width."),
            ("tb-border", "Thumbnail border."),
            ("tb-border-radius", "Thumbnail corner radius."),
            (
                "tb-selected-border-color",
                "Border colour of the selected thumbnail.",
            ),
            ("icon-width", "Left/right arrow width. Default `44px`."),
            ("left-icon", "URL of the left arrow image."),
            ("right-icon", "URL of the right arrow image."),
            ("border-radius", "Corner radius of the slides."),
        ],
        MjmlTag::MjCarouselImage => &[
            ("src", "**Required.** Slide image URL."),
            ("alt", "Alternative text."),
            ("href", "Wrap the slide in a link to this URL."),
            ("target", "Link target."),
            ("rel", "Link `rel` attribute."),
            ("thumbnails-src", "Separate image used for the thumbnail."),
            ("title", "Image `title` attribute."),
        ],
        MjmlTag::MjAccordion => &[
            (
                "border",
                "Border around the accordion. Default `2px solid black`.",
            ),
            ("icon-align", "`middle` (default), `top` or `bottom`."),
            ("icon-height", "Toggle icon height. Default `32px`."),
            ("icon-width", "Toggle icon width. Default `32px`."),
            ("icon-wrapped-url", "Icon shown while collapsed."),
            ("icon-unwrapped-url", "Icon shown while expanded."),
            ("icon-position", "`right` (default) or `left`."),
            ("font-family", "Font stack."),
        ],
        MjmlTag::MjAccordionElement => &[
            ("background-color", "Background colour of this entry."),
            ("border", "Border around this entry."),
            ("font-family", "Font stack."),
            ("icon-position", "`right` (default) or `left`."),
        ],
        MjmlTag::MjAccordionTitle => &[
            ("background-color", "Header background colour."),
            ("color", "Header text colour."),
            ("font-family", "Font stack."),
            ("font-size", "Font size. Default `13px`."),
            ("padding", "Header padding."),
        ],
        MjmlTag::MjAccordionText => &[
            ("background-color", "Body background colour."),
            ("color", "Body text colour."),
            ("font-family", "Font stack."),
            ("font-size", "Font size. Default `13px`."),
            ("line-height", "Line height."),
            ("letter-spacing", "Letter spacing."),
            ("padding", "Body padding."),
        ],
        MjmlTag::MjInclude => &[(
            "path",
            "**Required.** Path of the MJML (or `type=\"html\"`) file to include.",
        )],
        _ => &[],
    }
}

/// Markdown documentation for `tag`: its summary plus its documented attributes.
#[must_use]
pub fn tag_markdown(tag: MjmlTag) -> String {
    let mut out = format!("**`<{}>`**", tag.as_str());
    if let Some(summary) = tag_summary(tag) {
        out.push_str("\n\n");
        out.push_str(summary);
    }
    let attributes = tag_attributes(tag);
    if !attributes.is_empty() {
        out.push_str("\n\n**Attributes**\n\n");
        for (name, description) in attributes {
            out.push_str(&format!("- `{name}` — {description}\n"));
        }
    }
    out.push_str("\n_Documentation is built in and non-exhaustive._");
    out
}

/// Markdown documentation for `attribute` on `tag`.
#[must_use]
pub fn attribute_markdown(tag: MjmlTag, attribute: &str) -> String {
    let mut out = format!("**`{attribute}`** on `<{}>`", tag.as_str());
    if let Some(description) = attribute_doc(tag, attribute) {
        out.push_str("\n\n");
        out.push_str(description);
    } else {
        out.push_str("\n\nNo built-in documentation for this attribute.");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_tag_has_a_summary() {
        for tag in [
            MjmlTag::Mjml,
            MjmlTag::MjHead,
            MjmlTag::MjBody,
            MjmlTag::MjSection,
            MjmlTag::MjColumn,
            MjmlTag::MjText,
            MjmlTag::MjImage,
        ] {
            assert!(tag_summary(tag).is_some(), "{tag:?} has no summary");
        }
        assert!(tag_summary(MjmlTag::Unknown).is_none());
    }

    #[test]
    fn image_documents_its_required_src() {
        assert!(
            attribute_doc(MjmlTag::MjImage, "src")
                .unwrap()
                .contains("Required")
        );
    }

    #[test]
    fn common_attributes_are_appended_to_body_components() {
        let names: Vec<_> = tag_attributes(MjmlTag::MjText)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert!(names.contains(&"color"));
        assert!(names.contains(&"css-class"));
        assert!(names.contains(&"padding"));
        // `mj-head` is not a visible component and gets no layout attributes.
        assert!(tag_attributes(MjmlTag::MjHead).is_empty());
    }

    #[test]
    fn attribute_names_are_unique_per_tag() {
        for tag in [MjmlTag::MjText, MjmlTag::MjButton, MjmlTag::MjSection] {
            let mut names: Vec<_> = tag_attributes(tag).into_iter().map(|(n, _)| n).collect();
            let before = names.len();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), before, "duplicate attribute on {tag:?}");
        }
    }

    #[test]
    fn markdown_mentions_the_tag_and_its_attributes() {
        let markdown = tag_markdown(MjmlTag::MjImage);
        assert!(markdown.contains("`<mj-image>`"));
        assert!(markdown.contains("`src`"));
        assert!(markdown.contains("non-exhaustive"));
    }
}
