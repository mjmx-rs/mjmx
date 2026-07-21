//! Per-component renderers.
//!
//! Each MJML component is a small module producing its HTML fragment and pushing its
//! head requirements into the [`RenderContext`](crate::context::RenderContext). The
//! constants embedded here (default widths, paddings, the reset boilerplate) are the
//! actual mjml 4.18.0 defaults; they are marked with `// mjml default:` comments so
//! they read as component defaults rather than fixture-specific magic.

mod body;
mod column;
mod section;
mod text;

pub use body::Body;
pub use column::Column;
pub use section::Section;
pub use text::Text;
