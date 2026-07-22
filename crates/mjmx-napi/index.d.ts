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
