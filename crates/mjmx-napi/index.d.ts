/**
 * Node.js bindings for mjmx — an MJML-to-HTML compiler written in Rust.
 */

/** Options controlling a render. Omitted fields use the Rust-side defaults. */
export interface RenderOptions {
  /** Responsive breakpoint in pixels. Default: 480. */
  breakpoint?: number;
  /** Pretty-print the rendered HTML. Default: false. */
  beautify?: boolean;
  /** Minify the rendered HTML. Takes precedence over `beautify`. Default: false. */
  minify?: boolean;
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
 * Does not resolve `<mj-include>` (a bare string has no base directory) —
 * use {@link renderFile} for that.
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
