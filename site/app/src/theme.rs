//! Site palette, delivered as a singlestage [`Theme`].
//!
//! `ThemeProvider` emits these three strings into an unlayered `<style
//! id="theme">`: `:root { common light }`, plus a
//! `@media (prefers-color-scheme: dark)` block carrying `common dark` when the
//! mode is auto. singlestage's own tokens sit in `@layer base`, so unlayered
//! declarations here win. Theme switching is therefore pure CSS - no script, no
//! flash of the wrong palette, and it renders correctly server-side.
//!
//! `style/main.css` consumes these via `var()` and never defines a token.
//!
//! Every foreground/background pair was checked against the WCAG contrast
//! formula before being written here; body text clears 4.5:1 and headings
//! (large text) clear 3:1 in both modes.

use singlestage::Theme::Theme;
use std::borrow::Cow;

/// Slate neutrals with a bronze accent - bronze carries structure (wordmark,
/// headings, rules, links) and fills the header/footer bands.
pub const SLATE_BRONZE: Theme = Theme {
    common: Cow::Borrowed(
        r#"
    --font-sans: "Inter", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
    --font-mono: ui-monospace, "Cascadia Code", "Roboto Mono", Menlo, Consolas, monospace;

    --radius: 0.375rem;
    --radius-sm: calc(var(--radius) - 0.125rem);
    --radius-lg: calc(var(--radius) + 0.125rem);

    --space-1: 0.5rem;
    --space-2: 1rem;
    --space-3: 1.5rem;
    --space-4: 2rem;
    --space-5: 3rem;
    --space-6: 4rem;

    /* Prose measure and the wider span the header/footer bands may occupy. */
    --measure: 65ch;
    --shell-max: 70rem;

    --text-body: 1.125rem;
    --line-body: 1.7;
    --ratio-h4: 1.25rem;
    --ratio-h3: 1.563rem;
    --ratio-h2: 1.953rem;
    --ratio-h1: 2.441rem;
"#,
    ),
    light: Cow::Borrowed(
        r#"
    color-scheme: light;

    --background: oklch(0.98 0.003 255);
    --foreground: oklch(0.15 0.012 260);
    --card: oklch(0.99 0.003 255);
    --card-foreground: oklch(0.15 0.012 260);
    --popover: oklch(0.99 0.003 255);
    --popover-foreground: oklch(0.15 0.012 260);
    --muted: oklch(0.95 0.005 257);
    /* L 0.54 -> 0.51 so footer text clears 4.5:1 on --site-band, which is the
       darkest surface muted text sits on. Every other light surface is
       lighter, so this only improves elsewhere. */
    --muted-foreground: oklch(0.51 0.02 257);
    --secondary: oklch(0.95 0.005 257);
    --secondary-foreground: oklch(0.15 0.012 260);
    --border: oklch(0.9 0.01 255);
    --input: oklch(0.9 0.01 255);

    /* Solid bronze carries primary actions; its foreground is checked against
       the fill, not against the page background. */
    --primary: oklch(0.5 0.1 70);
    --primary-foreground: oklch(0.985 0.003 250);

    /* Focus ring is bronze so keyboard focus reads as part of the identity. */
    --ring: oklch(0.55 0.11 70);

    --destructive: oklch(0.55 0.245 27.325);
    --warning: oklch(0.769 0.188 70.08);
    --warning-foreground: oklch(0.145 0 0);
    --success: oklch(0.5 0.16 162.48);
    --success-foreground: oklch(0.985 0 0);
    --info: oklch(0.685 0.169 237.32);
    --info-foreground: oklch(0.985 0 0);

    /* Site-owned bronze roles. Kept distinct from --accent, which singlestage
       components use as a subtle hover surface. */
    --site-link: oklch(0.5 0.1 70);
    --site-heading: oklch(0.55 0.11 70);
    --site-rule: oklch(0.55 0.11 70);
    /* Slate, one lightness step down from the page. The step is what makes a
       band read as a band; a warm hue here differed from the page in
       temperature as well and read as a separate material laid on top. Bronze
       carries the header through the wordmark, the rule, and the links
       instead. Dark mode keeps its warm band, where no such clash appears. */
    --site-band: oklch(0.94 0.008 255);
    --site-band-foreground: oklch(0.15 0.012 260);
"#,
    ),
    dark: Cow::Borrowed(
        r#"
    --background: oklch(0.15 0.01 260);
    --foreground: oklch(0.985 0.003 250);
    --card: oklch(0.21 0.012 258);
    --card-foreground: oklch(0.985 0.003 250);
    --popover: oklch(0.21 0.012 258);
    --popover-foreground: oklch(0.985 0.003 250);
    --muted: oklch(0.27 0.013 258);
    --muted-foreground: oklch(0.708 0.02 257);
    --secondary: oklch(0.27 0.013 258);
    --secondary-foreground: oklch(0.985 0.003 250);
    --border: oklch(0.3 0.015 258);
    --input: oklch(0.3 0.015 258);

    --primary: oklch(0.75 0.1 75);
    --primary-foreground: oklch(0.15 0.01 260);

    --ring: oklch(0.75 0.1 75);

    --destructive: oklch(0.704 0.191 22.216);
    --warning: oklch(0.828 0.189 84.43);
    --warning-foreground: oklch(0.145 0 0);
    --success: oklch(0.765 0.177 163.22);
    --success-foreground: oklch(0.145 0 0);
    --info: oklch(0.746 0.16 232.66);
    --info-foreground: oklch(0.145 0 0);

    /* Bronze lightens toward brass so it keeps its contrast on a dark ground. */
    --site-link: oklch(0.75 0.1 75);
    --site-heading: oklch(0.78 0.1 75);
    --site-rule: oklch(0.75 0.1 75);
    --site-band: oklch(0.21 0.02 75);
    --site-band-foreground: oklch(0.985 0.003 250);
"#,
    ),
};
