# Backend integration patches

## Reddit previews

### Image and original-language follow-up

[reddit-language-images.patch](reddit-language-images.patch) is the current fix
in [backend PR #46](https://github.com/Papo-Chat/papo-backend/pull/46), based on
`aa0a4b99331fb48d6ac131a1c8b383f496d7c093` after PR #45 was merged. Apply this
follow-up to that baseline; do not reapply the original provider patch below.

It accepts the exact `cf.preview.redd.it` CDN and ignores community icons before
the post title. If Reddit marks the requested post as automatically translated,
the provider obtains the original title from official oEmbed and retains its
images/video. It does not force Portuguese or another fixed language. Because
oEmbed has no original post body, a translated body is omitted instead of
displayed as original text. Untranslated bodies remain available.

Migration `019_reddit_original_previews_cache.sql` expires older Reddit previews
once without deleting their media, shared IDs or message links. Run normal
backend migrations during deployment. The next share/refetch regenerates the
same entry and updates its linked messages; this does not bulk-fetch old posts.

```sh
git apply --check /path/to/papo-gtk/docs/backend/reddit-language-images.patch
git apply /path/to/papo-gtk/docs/backend/reddit-language-images.patch
cd backend
# Configure TEST_DATABASE_URL for a disposable PostgreSQL instance.
go test -race ./internal/services -run '^TestReddit' -count=1 -v
PAPO_REDDIT_LIVE=1 go test ./internal/services -run '^TestRedditLive' -count=1 -v
```

Go 1.26.10/PostgreSQL 16 validation: 1,597 tests/subtests passed, build/vet passed,
and focused race checks passed. The three opt-in sample/network tests skip in
the default full run; database suites executed. Live checks downloaded and
stored both supplied post images and verified the original English and
Portuguese titles. The earlier three public text/image/video examples and the
authorized HTTP 206 MP4 relay also passed. Fixtures additionally cover French
titles, exact CDN host validation and preservation of message links/cache scope.
The [upstream CI run](https://github.com/Papo-Chat/papo-backend/actions/runs/41766483760)
passed build, tests, coverage upload and vet for PR #46.

### Original provider (merged PR #45)

[reddit-previews.patch](reddit-previews.patch) contains the replacement provider
submitted in [backend PR #45](https://github.com/Papo-Chat/papo-backend/pull/45).
It is based on `fc3eae7df417a31a42d2a961a5f86a45f7bd934d`, after the maintainer
reverted the earlier scraper and redirect patch. The previous
`reddit-share-links.patch` is superseded and has been removed.

The new implementation reads Reddit's own `embed.reddit.com` document, extracts
post text/author/image and packaged MP4 metadata without running JavaScript,
and falls back to the official oEmbed endpoint if the document is unavailable
or unrecognized. The normal website's HTTP 200 challenge page is not post data.
It resolves opaque share links from headers, retains original cache keys,
restricts document/media redirects, and preserves the existing transport,
thumbnail processing, channel authorization and rate limits.

Packaged H.264 MP4s contain video and audio together. Expiring video signatures
are renewed before playback after the channel-read check, while ordinary cache
hits and subsequent range requests reuse existing data. One preview image is
exposed; full gallery browsing and live counts are not implemented. If both
official sources are unavailable from the deployment host, there is no preview.

Development used the isolated `target/backend-reddit-pr` clone; the adjacent
backend checkout remains unchanged. The patch includes `REDDIT_PREVIEWS.md`
with implementation notes, limits and deployment-host verification commands.

From the backend repository root, review and apply with:

```sh
git apply --check /path/to/papo-gtk/docs/backend/reddit-previews.patch
git apply /path/to/papo-gtk/docs/backend/reddit-previews.patch
cd backend
# Set TEST_DATABASE_URL to a disposable PostgreSQL test instance first.
go test -race ./internal/services -run '^TestReddit' -count=1 -v
go test ./... -count=1
go build ./...
go vet ./...
```

Validation used Go 1.26.10 and isolated PostgreSQL 16: all 15 Reddit tests passed
with the race detector (including opt-in recorded/live checks), the full suite
passed 1,590 tests/subtests, and build/vet passed. Database suites executed; the
two optional external/sample checks skip in the default full-suite run.
The [upstream GitHub CI run](https://github.com/Papo-Chat/papo-backend/actions/runs/41764137989)
also passed build, tests, coverage upload and vet for PR #45.

Live checks verified the Factorio share link (image/video), Reddit's embedding
announcement (body text) and the documented duck image post (thumbnail). The
actual video also returned HTTP 206 with MP4 bytes through the authorized relay.
A public sample MP4 range contained H.264 video and AAC audio in ffprobe.
No Papo production server, user session or private messages were accessed.

Run the public-network check with a disposable test database on the deployment
host, since upstream access can differ by IP:

```sh
PAPO_REDDIT_LIVE=1 go test ./internal/services -run '^TestRedditLivePublicPreviews$' -count=1 -v
```

Deploy through the backend's normal process after review. Existing messages
whose embed creation failed may need to be sent again, or refreshed through a
backend-supported regeneration mechanism. The GTK app never scrapes Reddit,
executes remote embed HTML or fabricates media missing from the API response.

The replacement does not depend on the old HTML scraper or its User-Agent
change. The GTK frontend already renders its existing `Embed` contract and uses
the backend's authorized media endpoints; it does not fetch Reddit pages.

## FxTwitter direct animations

Historical patch against `3252cfc2b0563d31d6c6eedd535c94fe5f15aceb`.
The updated backend already includes its own X proxy animation support; do not
apply this older patch blindly to current main. The commands and results below
describe the original review at that base revision.

[fxtwitter-animations.patch](fxtwitter-animations.patch) adds routing for the exact
`gif.fxtwitter.com` CDN and its `/tweet_video/<id>.webp` / `.gif` URLs. The
provided WebP example returned 297,574 bytes; its corresponding `.gif` URL
returned a 1,354,267-byte GIF. The patch selects that GIF rendition and uses the
existing bounded download, media storage, authorized embed endpoint and URL
cache. It does not introduce another page scraper or expose raw media paths.

Apply and review it from the backend repository root:

```sh
git apply --check /path/to/papo-gtk/docs/backend/fxtwitter-animations.patch
git apply /path/to/papo-gtk/docs/backend/fxtwitter-animations.patch
cd backend
go test ./internal/services -run '^TestFxTwitter' -count=1 -v
```

The package command requires the backend's normal test PostgreSQL configuration;
its `TestMain` otherwise skips tests. The patch's two tests were also executed
against all production service Go files without that database-dependent test
harness. They check rendition selection and reject spoofed hosts, credentials,
unexpected paths and unsafe schemes. This historical patch has not been applied
to the adjacent checkout.

GTK can now decode backend-supplied animated WebP and APNG alongside GIF, with
the same size/frame/allocation limits and motion preferences. A direct CDN link
still needs backend embed metadata and authorized bytes; client-side decoding
does not bypass that requirement. Use the current backend's automatic embed
pipeline for direct FxTwitter links.
