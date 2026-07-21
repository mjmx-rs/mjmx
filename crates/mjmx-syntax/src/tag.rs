//! The [`MjmlTag`] enum: the closed set of tag kinds `mjmx` understands.
//!
//! Each real MJML component maps to a canonical kebab-case tag string (`mj-section`,
//! `mj-social-element`, ...). Three pseudo-variants ([`MjmlTag::Comment`],
//! [`MjmlTag::Text`], [`MjmlTag::Unknown`]) model non-element nodes and unrecognized
//! input; they use `#`-prefixed sentinel strings and are never produced by
//! [`MjmlTag::from_str`].

/// A known MJML tag kind.
///
/// Real components round-trip through [`MjmlTag::as_str`] / [`MjmlTag::from_str`].
/// The `#`-prefixed pseudo-variants ([`Comment`](MjmlTag::Comment),
/// [`Text`](MjmlTag::Text), [`Unknown`](MjmlTag::Unknown)) model comment/text nodes
/// and unrecognized tags and are intentionally not parseable from a string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MjmlTag {
    /// `<mjml>` document root.
    Mjml,
    /// `<mj-head>`.
    MjHead,
    /// `<mj-body>`.
    MjBody,
    /// `<mj-attributes>`.
    MjAttributes,
    /// `<mj-breakpoint>`.
    MjBreakpoint,
    /// `<mj-font>`.
    MjFont,
    /// `<mj-preview>`.
    MjPreview,
    /// `<mj-style>`.
    MjStyle,
    /// `<mj-title>`.
    MjTitle,
    /// `<mj-html-attributes>`.
    MjHtmlAttributes,
    /// `<mj-section>`.
    MjSection,
    /// `<mj-column>`.
    MjColumn,
    /// `<mj-group>`.
    MjGroup,
    /// `<mj-wrapper>`.
    MjWrapper,
    /// `<mj-text>`.
    MjText,
    /// `<mj-button>`.
    MjButton,
    /// `<mj-image>`.
    MjImage,
    /// `<mj-divider>`.
    MjDivider,
    /// `<mj-spacer>`.
    MjSpacer,
    /// `<mj-raw>`.
    MjRaw,
    /// `<mj-table>`.
    MjTable,
    /// `<mj-social>`.
    MjSocial,
    /// `<mj-social-element>`.
    MjSocialElement,
    /// `<mj-navbar>`.
    MjNavbar,
    /// `<mj-navbar-link>`.
    MjNavbarLink,
    /// `<mj-hero>`.
    MjHero,
    /// `<mj-carousel>`.
    MjCarousel,
    /// `<mj-carousel-image>`.
    MjCarouselImage,
    /// `<mj-accordion>`.
    MjAccordion,
    /// `<mj-accordion-element>`.
    MjAccordionElement,
    /// `<mj-accordion-title>`.
    MjAccordionTitle,
    /// `<mj-accordion-text>`.
    MjAccordionText,
    /// `<mj-include>`.
    MjInclude,
    /// A comment node (`<!-- ... -->`). Pseudo-tag `#comment`.
    Comment,
    /// A text node. Pseudo-tag `#text`.
    Text,
    /// An unrecognized tag. Pseudo-tag `#unknown`.
    Unknown,
}

impl MjmlTag {
    /// The canonical tag string for this kind.
    ///
    /// Real components return their kebab-case tag (`"mj-section"`); the
    /// pseudo-variants return their `#`-prefixed sentinel (`"#comment"`).
    pub const fn as_str(&self) -> &'static str {
        match self {
            MjmlTag::Mjml => "mjml",
            MjmlTag::MjHead => "mj-head",
            MjmlTag::MjBody => "mj-body",
            MjmlTag::MjAttributes => "mj-attributes",
            MjmlTag::MjBreakpoint => "mj-breakpoint",
            MjmlTag::MjFont => "mj-font",
            MjmlTag::MjPreview => "mj-preview",
            MjmlTag::MjStyle => "mj-style",
            MjmlTag::MjTitle => "mj-title",
            MjmlTag::MjHtmlAttributes => "mj-html-attributes",
            MjmlTag::MjSection => "mj-section",
            MjmlTag::MjColumn => "mj-column",
            MjmlTag::MjGroup => "mj-group",
            MjmlTag::MjWrapper => "mj-wrapper",
            MjmlTag::MjText => "mj-text",
            MjmlTag::MjButton => "mj-button",
            MjmlTag::MjImage => "mj-image",
            MjmlTag::MjDivider => "mj-divider",
            MjmlTag::MjSpacer => "mj-spacer",
            MjmlTag::MjRaw => "mj-raw",
            MjmlTag::MjTable => "mj-table",
            MjmlTag::MjSocial => "mj-social",
            MjmlTag::MjSocialElement => "mj-social-element",
            MjmlTag::MjNavbar => "mj-navbar",
            MjmlTag::MjNavbarLink => "mj-navbar-link",
            MjmlTag::MjHero => "mj-hero",
            MjmlTag::MjCarousel => "mj-carousel",
            MjmlTag::MjCarouselImage => "mj-carousel-image",
            MjmlTag::MjAccordion => "mj-accordion",
            MjmlTag::MjAccordionElement => "mj-accordion-element",
            MjmlTag::MjAccordionTitle => "mj-accordion-title",
            MjmlTag::MjAccordionText => "mj-accordion-text",
            MjmlTag::MjInclude => "mj-include",
            MjmlTag::Comment => "#comment",
            MjmlTag::Text => "#text",
            MjmlTag::Unknown => "#unknown",
        }
    }

    /// Parse a tag string into its [`MjmlTag`] kind.
    ///
    /// This is the inverse of [`MjmlTag::as_str`] for real components. The
    /// `#`-prefixed pseudo-variants and any unrecognized input return `None`.
    ///
    /// Named `from_str` (not the [`std::str::FromStr`] trait) deliberately: it is
    /// infallible-shaped (`Option`, not `Result`) and partial (rejects pseudo-tags).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        let tag = match s {
            "mjml" => MjmlTag::Mjml,
            "mj-head" => MjmlTag::MjHead,
            "mj-body" => MjmlTag::MjBody,
            "mj-attributes" => MjmlTag::MjAttributes,
            "mj-breakpoint" => MjmlTag::MjBreakpoint,
            "mj-font" => MjmlTag::MjFont,
            "mj-preview" => MjmlTag::MjPreview,
            "mj-style" => MjmlTag::MjStyle,
            "mj-title" => MjmlTag::MjTitle,
            "mj-html-attributes" => MjmlTag::MjHtmlAttributes,
            "mj-section" => MjmlTag::MjSection,
            "mj-column" => MjmlTag::MjColumn,
            "mj-group" => MjmlTag::MjGroup,
            "mj-wrapper" => MjmlTag::MjWrapper,
            "mj-text" => MjmlTag::MjText,
            "mj-button" => MjmlTag::MjButton,
            "mj-image" => MjmlTag::MjImage,
            "mj-divider" => MjmlTag::MjDivider,
            "mj-spacer" => MjmlTag::MjSpacer,
            "mj-raw" => MjmlTag::MjRaw,
            "mj-table" => MjmlTag::MjTable,
            "mj-social" => MjmlTag::MjSocial,
            "mj-social-element" => MjmlTag::MjSocialElement,
            "mj-navbar" => MjmlTag::MjNavbar,
            "mj-navbar-link" => MjmlTag::MjNavbarLink,
            "mj-hero" => MjmlTag::MjHero,
            "mj-carousel" => MjmlTag::MjCarousel,
            "mj-carousel-image" => MjmlTag::MjCarouselImage,
            "mj-accordion" => MjmlTag::MjAccordion,
            "mj-accordion-element" => MjmlTag::MjAccordionElement,
            "mj-accordion-title" => MjmlTag::MjAccordionTitle,
            "mj-accordion-text" => MjmlTag::MjAccordionText,
            "mj-include" => MjmlTag::MjInclude,
            _ => return None,
        };
        Some(tag)
    }

    /// Whether this tag is a `<mj-head>` child (document metadata / config).
    pub const fn is_head_element(&self) -> bool {
        matches!(
            self,
            MjmlTag::MjHead
                | MjmlTag::MjAttributes
                | MjmlTag::MjBreakpoint
                | MjmlTag::MjFont
                | MjmlTag::MjPreview
                | MjmlTag::MjStyle
                | MjmlTag::MjTitle
                | MjmlTag::MjHtmlAttributes
        )
    }

    /// Whether this tag is a `<mj-body>` layout or content component.
    pub const fn is_body_element(&self) -> bool {
        matches!(
            self,
            MjmlTag::MjBody
                | MjmlTag::MjSection
                | MjmlTag::MjColumn
                | MjmlTag::MjGroup
                | MjmlTag::MjWrapper
                | MjmlTag::MjText
                | MjmlTag::MjButton
                | MjmlTag::MjImage
                | MjmlTag::MjDivider
                | MjmlTag::MjSpacer
                | MjmlTag::MjRaw
                | MjmlTag::MjTable
                | MjmlTag::MjSocial
                | MjmlTag::MjSocialElement
                | MjmlTag::MjNavbar
                | MjmlTag::MjNavbarLink
                | MjmlTag::MjHero
                | MjmlTag::MjCarousel
                | MjmlTag::MjCarouselImage
                | MjmlTag::MjAccordion
                | MjmlTag::MjAccordionElement
                | MjmlTag::MjAccordionTitle
                | MjmlTag::MjAccordionText
        )
    }

    /// Whether this tag renders raw/leaf content instead of parsing MJML children.
    ///
    /// These "ending" components carry HTML/text bodies that are emitted verbatim
    /// rather than descended into as further MJML nodes. Best-effort classification.
    pub const fn is_ending_tag(&self) -> bool {
        matches!(
            self,
            MjmlTag::MjText
                | MjmlTag::MjButton
                | MjmlTag::MjImage
                | MjmlTag::MjDivider
                | MjmlTag::MjSpacer
                | MjmlTag::MjRaw
                | MjmlTag::MjTable
                | MjmlTag::MjSocialElement
                | MjmlTag::MjNavbarLink
                | MjmlTag::MjAccordionText
                | MjmlTag::MjAccordionTitle
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_from_str_round_trip() {
        let variants = [
            MjmlTag::Mjml,
            MjmlTag::MjBody,
            MjmlTag::MjSection,
            MjmlTag::MjSocialElement,
            MjmlTag::MjAccordionText,
            MjmlTag::MjInclude,
        ];
        for tag in variants {
            assert_eq!(MjmlTag::from_str(tag.as_str()), Some(tag));
        }
    }

    #[test]
    fn known_strings_map() {
        assert_eq!(MjmlTag::from_str("mj-section"), Some(MjmlTag::MjSection));
        assert_eq!(MjmlTag::Mjml.as_str(), "mjml");
        assert_eq!(MjmlTag::MjBody.as_str(), "mj-body");
        assert_eq!(MjmlTag::MjSocialElement.as_str(), "mj-social-element");
    }

    #[test]
    fn unknown_and_pseudo_return_none() {
        assert_eq!(MjmlTag::from_str("nope"), None);
        assert_eq!(MjmlTag::from_str("#comment"), None);
        assert_eq!(MjmlTag::from_str("#text"), None);
        assert_eq!(MjmlTag::from_str("#unknown"), None);
    }

    #[test]
    fn pseudo_variant_strings() {
        assert_eq!(MjmlTag::Comment.as_str(), "#comment");
        assert_eq!(MjmlTag::Text.as_str(), "#text");
        assert_eq!(MjmlTag::Unknown.as_str(), "#unknown");
    }

    #[test]
    fn classification_spot_checks() {
        assert!(MjmlTag::MjPreview.is_head_element());
        assert!(!MjmlTag::MjSection.is_head_element());

        assert!(MjmlTag::MjColumn.is_body_element());
        assert!(!MjmlTag::MjHead.is_body_element());

        assert!(MjmlTag::MjText.is_ending_tag());
        assert!(MjmlTag::MjRaw.is_ending_tag());
        assert!(!MjmlTag::MjSection.is_ending_tag());
    }
}
