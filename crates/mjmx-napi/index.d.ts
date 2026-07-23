/**
 * Node.js bindings for mjmx — an MJML-to-HTML compiler written in Rust.
 */

/**
 * Options controlling a render. Omitted fields use the Rust-side defaults.
 *
 * The names are mjml's own `mjml2html` option names, so this reads as a drop-in for
 * JS mjml. Each field says whether it matches an mjml option or is an mjmx extension.
 */
export interface RenderOptions {
  /**
   * Which mjml release to reproduce. mjml 5 changed the rendered HTML (the `<body>`
   * tag moved under `mj-body`, minification switched to htmlnano/cssnano), so pass
   * `4` to keep the pre-5.x output. Default: 5.
   *
   * mjmx extension — a given mjml install *is* one version.
   */
  mjmlVersion?: 4 | 5;
  /**
   * Responsive breakpoint in pixels; columns collapse below this width. Default: 480.
   *
   * mjmx extension — mjml has no such option, it hardcodes 480 and only
   * `<mj-breakpoint>` changes it.
   */
  breakpoint?: number;
  /** Pretty-print the rendered HTML. Matches mjml's `beautify`. Default: false. */
  beautify?: boolean;
  /**
   * Minify the rendered HTML. Takes precedence over `beautify`.
   * Matches mjml's `minify`. Default: false.
   */
  minify?: boolean;
  /**
   * Keep MJML source comments in the output HTML.
   * Matches mjml's `keepComments`. Default: true.
   */
  keepComments?: boolean;
  /**
   * How hard to insist the document validates: report and render anyway (`'soft'`),
   * throw on any error (`'strict'`), or do not validate (`'skip'`).
   * Matches mjml's `validationLevel`. Default: `'soft'`.
   */
  validationLevel?: 'soft' | 'strict' | 'skip';
  /**
   * Directory (or file) that relative `<mj-include>` paths resolve against. Matches
   * mjml's `filePath`; it is what gives {@link render} — which otherwise has no base
   * directory — the ability to resolve includes, and it overrides the input file's own
   * directory in {@link renderFile}.
   */
  filePath?: string;
  /**
   * Web fonts to make importable, as mjml's `{ name: url }` object. Matches mjml's
   * `fonts`, including its *replace, never merge* semantics: passing a table drops the
   * five built-in Google fonts. Key order is preserved. A face is only emitted when the
   * body actually references it. Default: the built-in table.
   */
  fonts?: Record<string, string>;
  /**
   * How to minify, consulted only when `minify` is true — mjml likewise only spreads
   * it into its html-minifier call under `minify`. Matches mjml's `minifyOptions`.
   * Default: `{}`.
   */
  minifyOptions?: MinifyOptions;
  /**
   * Skip `<mj-include>` tags rather than resolving them. They are *dropped*, along
   * with the content they would have pulled in — mjml's parser returns before the
   * node is built. Matches mjml's `ignoreIncludes`.
   *
   * Omit to follow {@link mjmlVersion}: mjml 5 disabled includes by default, so `5`
   * ignores and `4` resolves. Setting it pins the choice regardless of version.
   */
  ignoreIncludes?: boolean;
  /**
   * Extra directories `<mj-include>` paths may resolve into, on top of the include
   * base. mjml 5 sandboxes includes to the base directory; a path resolving outside it
   * is denied and replaced with an `<!-- mj-include denied -->` comment. Each entry
   * widens the allowlist by one more root. Relative entries resolve against the include
   * base. Matches mjml's `includePath`.
   */
  includePath?: string[];
  /**
   * Also emit the responsive column rules inside an `@media only print` block, so a
   * printed email keeps its desktop layout. Matches mjml's `printerSupport`
   * (undocumented upstream but present in mjml-core's `helpers/mediaQueries.js`).
   * Default: false.
   */
  printerSupport?: boolean;
  /**
   * How to inline `<mj-style inline="inline">`, consulted only when the document has
   * an inline `<mj-style>` bucket. Matches mjml's `juiceOptions` — but only the keys
   * that mean something for mjmx's hand-rolled inliner, since there is no juice here
   * to forward an object to. This interface is closed on purpose: an unsupported
   * juice option is a type error rather than a silently ignored setting.
   * Default: `{}`.
   */
  juiceOptions?: JuiceOptions;
  /**
   * Templating delimiters the CSS inliner must not look inside, keyed by name.
   * Matches mjml's `juicePreserveTags`. These *add* to the built-ins, which are
   * always active — EJS (`<% %>`) and Handlebars (`{{ }}`) — and re-using a built-in
   * name replaces it in place. Key order is preserved, since masking is sequential.
   *
   * Unlike mjml, which assigns these into juice's module-global `codeBlocks` object,
   * these are per-call: they never leak into a later `render`. Default: `{}`.
   */
  juicePreserveTags?: Record<string, PreserveTag>;

  /**
   * Host-registered **custom components** (Tier 1), keyed by tag name. When rendering
   * reaches a tag the parser does not recognize, its callback renders it; an
   * unregistered unknown tag is the usual fatal "not a recognized MJML component"
   * error. A registered component is a body leaf element — it receives its resolved
   * attributes and inner text and returns HTML; it does not participate in the
   * responsive column layout (that is Tier 2, not yet available).
   *
   * mjmx's own extension point, not mjml's `.mjmlconfig` packages: a small, versioned
   * contract this compiler owns. Default: none.
   *
   * @example
   * render(src, {
   *   components: {
   *     "mj-badge": (cx) => `<span>${cx.attr("label") ?? ""}</span>`,
   *     "mj-card": (cx) => ({
   *       html: `<div class="card">${cx.rawInner}</div>`,
   *       styles: [".card{border:1px solid #ccc}"],
   *       fonts: [["Inter", "https://fonts.example/inter.css"]],
   *     }),
   *   },
   * });
   */
  components?: Record<string, ComponentFn>;
}

/** A custom-component renderer: attributes and inner text in, an HTML fragment out. */
export type ComponentFn = (cx: ComponentCx) => string | ComponentResult;

/**
 * The context a {@link ComponentFn} receives. Read-only: a snapshot of this element,
 * with no path back into the renderer.
 */
export interface ComponentCx {
  /** A cascade-resolved attribute (`mj-attributes` / `mj-class` / defaults applied). */
  attr(name: string): string | undefined;
  /** Every cascade-resolved attribute as a `{ name: value }` map. */
  attrs: Record<string, string>;
  /** The content-box width, in pixels, available to this element. */
  containerWidth: number;
  /** The active responsive breakpoint, in pixels. */
  breakpoint: number;
  /** The verbatim inner source text between the tag's open and close. */
  rawInner: string;
}

/**
 * What a {@link ComponentFn} may return instead of a bare HTML string, to also
 * contribute to `<head>`.
 */
export interface ComponentResult {
  /** The HTML fragment this component renders. */
  html: string;
  /** `<style>` blocks to add to the head (deduped). */
  styles?: string[];
  /**
   * Web fonts to add to the head, each a `[name, href]` pair. As with mjml's fonts, a
   * font is emitted only when its family `name` is referenced in the rendered body.
   */
  fonts?: [name: string, href: string][];
}

/**
 * mjml's `juiceOptions`, restricted to what mjmx's inliner can honour. Omitted
 * fields keep juice's own defaults, as mjml configures them.
 *
 * The juice options deliberately **absent** here, each for a stated reason:
 *
 * - `extraCss`, `applyStyleTags`, `removeStyleTags`, `insertPreservedExtraCss` —
 *   in mjmx these are not settings but the shape of the pass. `mjmx` splits
 *   `<mj-style>` into an inline bucket and a `<style>` bucket before rendering, and
 *   the inliner only ever sees the first. (Upstream lets you override them only
 *   because `...juiceOptions` is spread last, which can delete a document's own
 *   inline CSS.)
 * - `preserveMediaQueries`, `preserveFontFaces`, `preserveKeyFrames`,
 *   `preservePseudos` — unreachable: juice reads them only under
 *   `insertPreservedExtraCss` / `removeStyleTags`, and mjml sets both `false`.
 * - `xmlMode`, `url`, `webResources` — no mjmx analogue; the inliner does no I/O.
 *   (`resolveCSSVariables` and `inlinePseudoElements` are both implemented — see the
 *   fields below.)
 *
 * **How the refusal is enforced here.** This interface is closed, so passing one of
 * those keys in an object literal is a compile error. It is *not* a runtime error:
 * N-API decodes this object field by field into a typed Rust struct, so a key it does
 * not know is dropped before any mjmx code runs, and plain-JS callers (or a
 * pre-typed variable, which escapes TypeScript's excess-property check) get it
 * silently ignored. The wasm build, which sees the raw object, throws instead. Same
 * trade-off as `minifyOptions` in this binding.
 */
export interface JuiceOptions {
  /**
   * Mirror a matched `width` declaration onto the `width` attribute of
   * `table`/`td`/`th`/`img`. Default: true.
   */
  applyWidthAttributes?: boolean;
  /**
   * Mirror a matched `height` declaration onto the `height` attribute of
   * `table`/`td`/`th`/`img`. Default: true.
   */
  applyHeightAttributes?: boolean;
  /**
   * Mirror `background-color`/`background-image`/`text-align`/`vertical-align` onto
   * `bgcolor`/`background`/`align`/`valign` on `<table>`. Default: true.
   */
  applyAttributesTableElements?: boolean;
  /**
   * Keep the `!important` keyword on an inlined declaration. Default: false, which is
   * juice's default and therefore mjml's: `!important` still decides the cascade
   * (it evicts the element's own declaration), but the keyword is dropped from the
   * written value. Attribute mirrors never carry it either way.
   */
  preserveImportant?: boolean;
  /**
   * Resolve `var(--x)` / `var(--x, fallback)` against the custom-property cascade and
   * drop the `--x` declarations. Default: true (juice's default).
   */
  resolveCSSVariables?: boolean;
  /**
   * Synthesize `::before`/`::after` from their `content` (string → `<span>`, `url()` →
   * `<img>`; `attr()`/`var()`/`counter()` resolved) and splice them in. Default: false
   * (juice's default; mjml never enables it).
   */
  inlinePseudoElements?: boolean;
  /** The attribute inline declarations are read from and written to. Default: "style". */
  styleAttributeName?: string;
}

/** The delimiters of one `juicePreserveTags` entry. */
export interface PreserveTag {
  /** The opening delimiter, e.g. `"<#"`. */
  start: string;
  /** The closing delimiter, e.g. `"</#"`. */
  end: string;
}

/**
 * mjml's `minifyOptions` — the four html-minifier switches mjml sets. Omitted fields
 * keep their defaults.
 */
export interface MinifyOptions {
  /** Collapse insignificant whitespace. Default: true (mjml's own). */
  collapseWhitespace?: boolean;
  /**
   * Minify CSS inside `<style>`. Default: false (mjml's own).
   *
   * mjmx applies a conservative whitespace/comment pass here, not html-minifier's
   * full `clean-css` optimizer: declarations and selectors are never rewritten.
   */
  minifyCSS?: boolean;
  /** Treat attribute names case-sensitively; false lower-cases them. Default: true. */
  caseSensitive?: boolean;
  /**
   * Drop empty `class`/`id`/`style`/`title`/`lang`/`dir`, empty event handlers and
   * `value=""` on `<input>` — html-minifier's exact allow-list.
   *
   * **Default: false, where mjml passes true.** mjml's own output carries
   * `<div … style="" role="article">`, so mjml's default removes an attribute during
   * minification; mjmx keeps `minify` semantics-preserving by default. Set this to
   * `true` for byte parity with mjml.
   */
  removeEmptyAttributes?: boolean;
}

/** Options controlling `format`. */
export interface FormatOptions {
  /** Spaces per indentation level. Default: 2. */
  indentWidth?: number;
}

/** A span-anchored label attached to a {@link Diagnostic}. */
export interface DiagnosticLabel {
  /** Start byte offset into the source (inclusive). */
  start: number;
  /** End byte offset into the source (exclusive). */
  end: number;
  /** Message describing the labeled region. */
  message: string;
}

/** A single diagnostic produced by the parser, linter, or renderer. */
export interface Diagnostic {
  severity: 'error' | 'warning' | 'advice';
  message: string;
  labels: DiagnosticLabel[];
  help?: string;
}

/**
 * An `Error` thrown by {@link render}, {@link renderFile} or {@link format},
 * carrying the structured diagnostics that caused the failure.
 */
export interface MjmxError extends Error {
  diagnostics?: Diagnostic[];
}

/**
 * Render MJML source text to responsive HTML.
 *
 * Resolves `<mj-include>` only when `options.filePath` supplies a base directory —
 * a bare string has none. Use {@link renderFile} to render a file, which derives that
 * base from the file itself.
 *
 * @throws {MjmxError} if parsing or rendering produced errors.
 */
export declare function render(source: string, options?: RenderOptions): string;

/**
 * Render the MJML file at `path`, resolving `<mj-include>` relative to it.
 *
 * @throws {MjmxError} on I/O, parse, include-resolution, or render errors.
 */
export declare function renderFile(path: string, options?: RenderOptions): string;

/** Lint MJML source, returning every diagnostic. Never throws. */
export declare function lint(source: string): Diagnostic[];

/**
 * Format MJML source into canonical MJML source text. Idempotent.
 *
 * @throws {MjmxError} if the source is malformed.
 */
export declare function format(source: string, options?: FormatOptions): string;

/** The version of the native addon. */
export declare function version(): string;
