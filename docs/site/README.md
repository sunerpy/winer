# winer's documentation site

The pages of <https://firlab.app/winer/>: Chinese at the root, English under `en/`, each page at the
same path in both. They live here, next to the code they describe, so a change to what a user sees
updates its page in the same pull request. The site itself (VitePress configuration, theme, build
checks) lives in [sunerpy/firlab](https://github.com/sunerpy/firlab) under `winer/`, whose
`scripts/sync-winer-docs.sh` copies these files and the mark (`app/src-tauri/app-icon.svg`) over.

## Writing

- Standard written Chinese and plain English; no colloquialisms, no names of winer's code.
- A page says what a user sees and does: the window's own labels in bold, keys in `code`.
- Every page exists in both languages; a link inside the site has no extension (`/guide/install`,
  `/en/guide/install`).
- Components the site registers: `HomeIndex`, `HomeSteps`, `SplitBlock`, `HomePrivacy`,
  `ScreenFigure`, `StatusTag`, and VitePress's `Badge`. The sync refuses any other tag.

## The home page

`index.md` and `en/index.md` carry the home page's data in a `home:` frontmatter block, checked at
build time by firlab's `winer/src/.vitepress/theme/data/home-schema.ts`: `facts` (two or more),
`visual.desktop`, `index` (feature groups, each item with a `status` of `available`, `experimental`,
`building` or `planned`), `steps`, `shots` (captures a `SplitBlock` names) and `privacy`.

## Screenshots

`public/screens/*.webp`, 1440 × 900, one light and one dark capture of each, taken from the demo
client (`pnpm dev`) so no real player appears: the light theme for light, Hextech for dark.

## Publishing

Two workflows connect this repository to the site:

- `.github/workflows/docs-site.yml` runs on pull requests that touch the pages or the mark. It syncs
  them into a checkout of firlab's `main`, builds the site and runs `check-dist.sh`, without a
  secret. It is advisory and not part of `CI Success`.
- `.github/workflows/publish-docs-site.yml` runs after a merge to `main` that touches them. It runs
  the same sync, commits the result as `docs(winer): sync from winer@<sha>` to the branch
  `winer-docs/sync` in firlab, and opens a pull request from it, or updates the one already open.
  Once firlab's checks pass, it squash-merges the pull request at the commit that was checked, and
  firlab's deploy publishes the site. A failed check leaves the pull request open; the next run
  replaces its commit.

`publish-docs-site.yml` needs the repository secret `FIRLAB_DOCS_TOKEN`: a fine-grained personal
access token for `sunerpy/firlab` only, with Contents and Pull requests read and write and nothing
else. GitHub has no API that creates one, so it is made by hand and stored with:

```sh
gh secret set FIRLAB_DOCS_TOKEN --repo sunerpy/winer
```

Without it the workflow fails at its first step and says so, and nothing reaches firlab until
someone opens a sync pull request there by hand. `gh workflow run publish-docs-site.yml --repo
sunerpy/winer` runs it again once the token is set.
