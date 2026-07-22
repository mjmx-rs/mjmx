//! The data-driven rule tables that drive the linter.
//!
//! Keeping the parent/child relationships as plain tables (rather than baking them
//! into the walker) makes new rules cheap to add: extend [`allowed_children`],
//! [`required_parent`], or [`required_attributes`] and the walker picks them up. The
//! tables mirror MJML 4's real component nesting rules.

use mjmx_syntax::MjmlTag;

/// The content components accepted directly inside a `<mj-column>` (and `<mj-hero>`).
///
/// These are the leaf/content-level MJML components — everything that renders visible
/// email content rather than layout structure.
const CONTENT: &[MjmlTag] = &[
    MjmlTag::MjText,
    MjmlTag::MjButton,
    MjmlTag::MjImage,
    MjmlTag::MjDivider,
    MjmlTag::MjSpacer,
    MjmlTag::MjTable,
    MjmlTag::MjSocial,
    MjmlTag::MjNavbar,
    MjmlTag::MjAccordion,
    MjmlTag::MjCarousel,
    MjmlTag::MjRaw,
];

/// The set of element tags allowed as direct children of `tag`.
///
/// Returns `None` for tags whose children are not structurally constrained — leaf /
/// ending components (`mj-text`, `mj-table`, ...) whose bodies are raw HTML/text, plus
/// containers like `mj-attributes` that accept arbitrary tag names. A `Some(&[])` set
/// would mean "no element children allowed at all"; we never need that today.
///
/// `mj-raw` is listed wherever raw pass-through markup is permitted, and `mj-include`
/// is treated as universally allowed by the walker (it expands to arbitrary content),
/// so it is intentionally omitted from these lists.
#[must_use]
pub fn allowed_children(tag: MjmlTag) -> Option<&'static [MjmlTag]> {
    use MjmlTag::{
        Comment, MjAccordion, MjAccordionElement, MjAccordionText, MjAccordionTitle, MjAttributes,
        MjBody, MjBreakpoint, MjButton, MjCarousel, MjCarouselImage, MjColumn, MjDivider, MjFont,
        MjGroup, MjHead, MjHero, MjHtmlAttribute, MjHtmlAttributes, MjImage, MjInclude, MjNavbar,
        MjNavbarLink, MjPreview, MjRaw, MjSection, MjSelector, MjSocial, MjSocialElement, MjSpacer,
        MjStyle, MjTable, MjText, MjTitle, MjWrapper, Mjml, Text, Unknown,
    };

    Some(match tag {
        // `<mj-raw>` is legal directly under `<mjml>`: mjml collects those as
        // `mjOutsideRaws` and emits the `position="file-start"` ones before the doctype.
        Mjml => &[MjHead, MjBody, MjRaw],
        MjBody => &[MjSection, MjWrapper, MjHero, MjRaw],
        MjWrapper => &[MjSection, MjRaw],
        MjSection => &[MjColumn, MjGroup, MjRaw],
        MjGroup => &[MjColumn, MjRaw],
        MjColumn | MjHero => CONTENT,
        MjSocial => &[MjSocialElement, MjRaw],
        MjNavbar => &[MjNavbarLink, MjRaw],
        MjCarousel => &[MjCarouselImage, MjRaw],
        MjAccordion => &[MjAccordionElement, MjRaw],
        MjAccordionElement => &[MjAccordionTitle, MjAccordionText],
        MjHead => &[
            MjAttributes,
            MjBreakpoint,
            MjFont,
            MjPreview,
            MjStyle,
            MjTitle,
            MjHtmlAttributes,
            MjRaw,
        ],
        // `<mj-html-attributes>` holds `<mj-selector path>`, each holding
        // `<mj-html-attribute name>` leaves.
        MjHtmlAttributes => &[MjSelector],
        MjSelector => &[MjHtmlAttribute],
        // Explicitly unconstrained: leaf/ending components and free-form containers.
        MjAttributes | MjBreakpoint | MjFont | MjPreview | MjStyle | MjTitle | MjHtmlAttribute
        | MjRaw | MjText | MjButton | MjImage | MjDivider | MjSpacer | MjTable
        | MjSocialElement | MjNavbarLink | MjCarouselImage | MjAccordionTitle | MjAccordionText
        | MjInclude | Comment | Text | Unknown => return None,
    })
}

/// The single container tag a "child-only" element must sit directly inside.
///
/// These components are meaningless outside their container and are validated from the
/// child's side (rather than every possible wrong parent listing them as disallowed).
#[must_use]
pub const fn required_parent(tag: MjmlTag) -> Option<MjmlTag> {
    Some(match tag {
        MjmlTag::MjSocialElement => MjmlTag::MjSocial,
        MjmlTag::MjNavbarLink => MjmlTag::MjNavbar,
        MjmlTag::MjCarouselImage => MjmlTag::MjCarousel,
        MjmlTag::MjAccordionElement => MjmlTag::MjAccordion,
        MjmlTag::MjAccordionTitle | MjmlTag::MjAccordionText => MjmlTag::MjAccordionElement,
        MjmlTag::MjSelector => MjmlTag::MjHtmlAttributes,
        MjmlTag::MjHtmlAttribute => MjmlTag::MjSelector,
        _ => return None,
    })
}

/// Attributes that a component ought to carry, at least one per group.
///
/// Each returned group is satisfied if **any** of its names is present, e.g.
/// `mj-social-element` needs `name` *or* `src`. Violations are warnings, not errors —
/// rendering still succeeds (often with a sensible default or an empty slot).
#[must_use]
pub fn required_attributes(tag: MjmlTag) -> &'static [&'static [&'static str]] {
    match tag {
        MjmlTag::MjImage => &[&["src"]],
        MjmlTag::MjSocialElement => &[&["name", "src"]],
        MjmlTag::MjCarouselImage => &[&["src"]],
        _ => &[],
    }
}
