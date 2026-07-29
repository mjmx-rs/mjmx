//! `<mj-body>` — the outer email wrapper `<div>`.

use mjmx_ast::Element;
use mjmx_syntax::Length;
use std::fmt::Write;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_children, render_element};

/// Renderer for `<mj-body>`.
pub struct Body<'a> {
    element: &'a Element<'a>,
}

impl<'a> Body<'a> {
    /// Wrap an `<mj-body>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Body<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // mj-body's own attributes. mjml declares exactly `width` (default 600px) and
        // `background-color` (no default) plus the universal `css-class`; all three are
        // resolved through the semantic layer so <mj-attributes>/mj-class apply.
        let width = ctx
            .semantic
            .attribute(self.element, "width", Some("600px"))
            .unwrap_or("600px");
        // `getChildContext` sets containerWidth from the resolved width; sections,
        // wrappers and columns all measure against it. Unparseable values keep 600px.
        if let Some(px) = Length::parse(width).map(|l| l.value as u32) {
            ctx.container_width = px;
        }
        let background_color = ctx
            .semantic
            .attribute(self.element, "background-color", None)
            .map(str::to_string);
        // mjml's `setBackgroundColor` — document assembly appends this to <body style>.
        ctx.body_background_color = background_color.clone();
        let css_class = ctx.semantic.attribute(self.element, "css-class", None);

        // A <mj-preview> emits a hidden preheader <div> as the first body child,
        // before the email wrapper.
        if let Some(preview) = ctx.semantic.head().preview.clone() {
            buf.push_str("<div style=\"display:none;font-size:1px;color:#ffffff;line-height:1px;max-height:0px;max-width:0px;opacity:0;overflow:hidden;\">");
            buf.push_str(&preview);
            buf.push_str("</div>");
        }

        // The body carries the ARIA email wrapper. Attribute order mirrors mjml's
        // `htmlAttributes` call: aria-label (only with a <mj-title>), aria-roledescription,
        // class (only with css-class), style, role, lang, dir. lang/dir are inherited from
        // the <mjml> root; `style` is empty unless a background-color is set.
        let aria_label = match &ctx.semantic.head().title {
            Some(title) => format!("aria-label=\"{title}\" "),
            None => String::new(),
        };
        if ctx.options.compat.is_v5() {
            // mjml 5 rebuilt this tag: `class` moved to the <body> element, `style` moved
            // to the end and now repeats the *whole* body style rather than carrying the
            // background-color alone (so the empty `style=""` mjml 4 always emitted is
            // gone). `id` is new in 5 and also lands on <body>. Both travel to document
            // assembly through the context, since <body> is written there.
            ctx.body_css_class = css_class.map(str::to_string);
            ctx.body_id = ctx
                .semantic
                .attribute(self.element, "id", None)
                .map(str::to_string);
            let style = match &background_color {
                Some(color) => format!("word-spacing:normal;background-color:{color};"),
                None => "word-spacing:normal;".to_owned(),
            };
            let _ = write!(
                buf,
                "<div {aria_label}aria-roledescription=\"email\" role=\"article\" lang=\"{lang}\" dir=\"{dir}\" style=\"{style}\">",
                lang = ctx.lang,
                dir = ctx.dir,
            );
        } else {
            let class = match css_class {
                Some(class) => format!("class=\"{class}\" "),
                None => String::new(),
            };
            let style = match &background_color {
                Some(color) => format!("background-color:{color};"),
                None => String::new(),
            };
            let _ = write!(
                buf,
                "<div {aria_label}aria-roledescription=\"email\" {class}style=\"{style}\" role=\"article\" lang=\"{lang}\" dir=\"{dir}\">",
                lang = ctx.lang,
                dir = ctx.dir,
            );
        }

        // Only element children (sections) and comments render; whitespace text between
        // body-level components is not significant and is skipped.
        render_children(self.element, ctx, buf, |child, ctx, buf| {
            render_element(child, ctx, buf)
        })?;

        buf.push_str("</div>");
        Ok(())
    }
}
