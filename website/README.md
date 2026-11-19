# CalcCraft's pages on rapalon.com

> **Draft for Konst's review, not legal advice.** `TERMS.md` was written for CalcCraft Pro's
> Play listing on November 19, 2026, and has not been reviewed by a lawyer. Its governing law
> is Delaware (Konst's choice of November 19, 2026), with no venue or arbitration clause
> beyond "courts of competent jurisdiction". Have the whole text checked before it goes live.

This repository is the source of truth for CalcCraft's pages on the Rapalon Labs website:

| Page | Source in this repository | Becomes, in rapalon/website |
| --- | --- | --- |
| <https://rapalon.com/calccraft/> | `website/calccraft.md` and `website/calccraft/*.png` | `src/content/projects/calccraft.md` and `src/content/projects/calccraft/` |
| <https://rapalon.com/calccraft/privacy/> | `PRIVACY.md` (repository root) | `src/content/policies/calccraft-privacy.md` |
| <https://rapalon.com/calccraft/terms/> | `TERMS.md` (repository root) | `src/content/policies/calccraft-terms.md` |

Edit these files here, never in rapalon/website: a sync from here overwrites them there.
rapalon/website keeps only the shared site and its templates (its `docs/product-pages.md`).

## The product page

`calccraft.md` is the product page: front matter validated by the `projects` collection's
schema (`src/content.config.ts` in rapalon/website; the fields are described in its README,
"Add a project"), then the description in Markdown. The screenshots sit next to it in
`calccraft/`, referenced as `./calccraft/<file>.png`, and the site's build optimises them.

They were copied verbatim from rapalon/website `main` at commit
`238cb48efb2027f1d68a23d2f31a631c3704da23` (November 17, 2026), the same bytes (SHA-256):

- `calccraft.md`: `218b86a7649d5035979b67a4ec8f79c7f4e56761c6b136df3b1a8287026e6123`
- `calccraft/standard.png`: `b6f424e8d5edcf932e77d7f3b3bcbee93420646338ac35020af3de8b432f3e20`
- `calccraft/scientific.png`: `44ec3e38aa7740bf26e42cdf6fe410497a7bfd20312562cf0aa8ff4cf26458ac`
- `calccraft/converter.png`: `136a6eebfba3d8b01b7308e52c42e52e12056efacf92dadbebab48b6647ffaf8`

## The legal pages

`PRIVACY.md` and `TERMS.md` stay at the repository root, where GitHub, F-Droid and the stores'
reviewers look for them, and they are the only copies in this repository. The site's build
can't read another repository, so the sync copies each one into rapalon/website's `policies`
collection: the file's text unchanged, below a front matter that names its page and the commit
it comes from.

```yaml
---
product: calccraft
kind: terms            # or privacy (the `kind` field is new: see "Needed in rapalon/website")
title: CalcCraft Terms of Service
source: https://github.com/rapalon/calccraft/blob/main/TERMS.md
sourceCommit: <40-character commit of this repository>
---
```

The copy rule: never edit a copy in rapalon/website; change the root file here, merge it, and
sync again. The app links the privacy policy (`PRIVACY_URL` in `src/app.rs`), so its URL must
not change.

Both texts are written for phones: short sections, lists instead of tables, and links named in
words (a long URL can't wrap and would make the page scroll sideways on a 360 px screen with the
site's current CSS). Keep it that way.

## Sync

`node website/sync.mjs <path to a rapalon/website checkout>` writes all of the above into that
checkout, with the front matter, from a clean checkout of the commit to publish (it refuses
uncommitted changes in these files). It needs Node only and changes nothing here.

What a CI job for this (not built yet) must do, on a push to `main` that changes `website/`,
`PRIVACY.md` or `TERMS.md`:

1. Check out rapalon/website `main` with a token that may push branches and open pull requests
   there (a fine-grained token or a GitHub App, stored as a secret of this repository).
2. Run `node website/sync.mjs <checkout>`.
3. If anything changed, commit it to a branch `sync/calccraft-<short commit>` and open a pull
   request against rapalon/website `main` that names this repository's commit, so the site's
   CI (`npm test`) checks it and a person merges it. Never push to its `main` directly.
4. Do nothing if the sync changed nothing.

Until that job exists, run the script by hand and open the pull request in rapalon/website.

## Needed in rapalon/website

Found by rendering the site locally (rapalon/website at `238cb48`) with Playwright at 360 × 800
and 412 × 915, device scale factor 2. Today's `/calccraft/privacy/` scrolls sideways on both:
its two tables have no styles, and the page's grid column grows to the table's 521 px. Body text
is 16 px with a line height of 1.65, and the back link is 44 px tall: those are fine. Section
headings have no space above them (`.product h2` sets `margin: 0 0 var(--space-2)`), so
sections run together.

The texts here no longer have tables or long URLs, so they fit phones even without the changes
below. The changes are still needed, for the next long word or table, and for the Terms page:

1. **The Terms page.** `src/content.config.ts`, collection `policies`: add
   `kind: z.enum(['privacy', 'terms']).default('privacy'),` after `product`. In
   `src/pages/[slug]/privacy.astro`, filter on it:
   `.filter((policy) => policy.data.kind === 'privacy' && slugs.has(policy.data.product))`
   (otherwise the terms file would also claim `/calccraft/privacy/`). Add
   `src/pages/[slug]/terms.astro`, a copy of `privacy.astro` that keeps `kind === 'terms'`,
   strips ` Terms of Service` from the title for the name, and has the description
   `` `The terms for using ${name}.` `` and image alt `` `${name}: terms of service` ``. In
   `src/pages/[slug].astro`, show a "Terms of Service" link next to "Privacy policy" when the
   product has a `terms` policy. Update `docs/product-pages.md` ("Policies") and the tests.
2. **Phones.** `src/styles/products.css`, after `.prose strong`:

   ```css
   .product > * {
     min-width: 0; /* a grid item never grows past the column */
   }

   .prose {
     overflow-wrap: anywhere; /* long URLs and code wrap */
   }

   .prose > h2:not(:first-child) {
     margin-top: var(--space-4); /* air above each section */
   }

   .prose table {
     display: block; /* a wide table scrolls in its own box */
     max-width: 100%;
     overflow-x: auto;
     border-collapse: collapse;
     margin: 0 0 var(--space-2);
   }

   .prose th,
   .prose td {
     padding: 0.4rem 0.75rem 0.4rem 0;
     border-bottom: 1px solid var(--line);
     text-align: left;
     vertical-align: top;
   }
   ```

With both, the privacy and terms pages had no sideways scrolling at either size, 16 px body
text and 40 px above each heading; the site's tests passed but for the five redirect tests that
need `wrangler dev`, which couldn't start on the machine used. Links inside sentences are 20 px
tall, like any text link; WCAG 2.5.8 exempts inline links from the 24 px target size, and the
1.65 line height keeps them apart.
