//! The `<mj-attributes>` registry: global, per-tag, and per-class default maps.
//!
//! `<mj-attributes>` lets a document set attribute defaults centrally:
//!
//! ```xml
//! <mj-attributes>
//!   <mj-all font-family="Georgia, serif" />   <!-- applies to every component -->
//!   <mj-text color="#ff0000" />               <!-- applies to every <mj-text> -->
//!   <mj-class name="blue" color="#0000ff" />  <!-- applies to mj-class="blue" -->
//! </mj-attributes>
//! ```
//!
//! Each row becomes a name→value map. Resolution precedence (highest first) is applied
//! by [`Semantic::attribute`](crate::Semantic::attribute).

use std::collections::HashMap;

use mrml_ast::Element;

/// A flat attribute default map (`name` → `value`).
type AttrMap<'a> = HashMap<&'a str, &'a str>;

/// The `<mj-attributes>` defaults collected from a document's head.
#[derive(Debug, Default, Clone)]
pub struct AttributesRegistry<'a> {
    /// `<mj-all ...>` — applies to every component.
    all: AttrMap<'a>,
    /// `<mj-text ...>`, `<mj-button ...>`, … keyed by the row's raw tag string.
    per_tag: HashMap<&'a str, AttrMap<'a>>,
    /// `<mj-class name="x" ...>` keyed by the class name.
    classes: HashMap<&'a str, AttrMap<'a>>,
}

impl<'a> AttributesRegistry<'a> {
    /// Ingest one `<mj-attributes>` element, folding its rows into the registry.
    ///
    /// Later rows merge into earlier ones (a repeated name overwrites), matching how
    /// mjml accumulates global attribute state.
    pub fn ingest(&mut self, mj_attributes: &Element<'a>) {
        for row in mj_attributes.element_children() {
            match row.raw_tag.as_str() {
                "mj-all" => collect_into(&mut self.all, row, None),
                "mj-class" => {
                    if let Some(name) = row.attr("name") {
                        let map = self.classes.entry(name).or_default();
                        // The `name` attribute is the selector, not a defaulted value.
                        collect_into(map, row, Some("name"));
                    }
                }
                other => {
                    let map = self.per_tag.entry(other).or_default();
                    collect_into(map, row, None);
                }
            }
        }
    }

    /// The global `<mj-all>` value for `name`, if any.
    #[must_use]
    pub fn all(&self, name: &str) -> Option<&'a str> {
        self.all.get(name).copied()
    }

    /// The per-tag default for `name` on elements whose raw tag is `tag`.
    #[must_use]
    pub fn per_tag(&self, tag: &str, name: &str) -> Option<&'a str> {
        self.per_tag.get(tag).and_then(|m| m.get(name)).copied()
    }

    /// The value for `name` from `mj-class="..."`, later classes winning over earlier.
    #[must_use]
    pub fn class(&self, classes: &str, name: &str) -> Option<&'a str> {
        let mut found = None;
        for class in classes.split_whitespace() {
            if let Some(v) = self.classes.get(class).and_then(|m| m.get(name)) {
                found = Some(*v);
            }
        }
        found
    }
}

/// Copy every valued attribute of `row` into `map`, skipping `skip` if given.
fn collect_into<'a>(map: &mut AttrMap<'a>, row: &Element<'a>, skip: Option<&str>) {
    for attr in &row.attributes {
        let name = attr.name.as_str();
        if Some(name) == skip {
            continue;
        }
        if let Some(value) = attr.value_str() {
            map.insert(name, value);
        }
    }
}
