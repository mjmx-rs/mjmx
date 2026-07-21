//! Per-component renderers.
//!
//! Each MJML component is a small module producing its HTML fragment and pushing its
//! head requirements into the [`RenderContext`](crate::context::RenderContext). The
//! constants embedded here (default widths, paddings, the reset boilerplate) are the
//! actual mjml 4.18.0 defaults; they are marked with `// mjml default:` comments so
//! they read as component defaults rather than fixture-specific magic.

mod body;
mod button;
mod column;
mod divider;
mod group;
mod image;
mod navbar;
mod navbar_link;
mod raw;
mod section;
mod social;
pub(crate) mod social_element;
mod spacer;
mod table;
mod text;
mod wrapper;

pub use body::Body;
pub use button::Button;
pub use column::Column;
pub use divider::Divider;
pub use group::Group;
pub use image::Image;
pub use navbar::Navbar;
pub use navbar_link::NavbarLink;
pub use raw::Raw;
pub use section::Section;
pub use social::Social;
pub use social_element::SocialElement;
pub use spacer::Spacer;
pub use table::Table;
pub use text::Text;
pub use wrapper::Wrapper;
