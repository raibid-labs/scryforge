# provider-miniflux

[Miniflux](https://miniflux.app) provider for Scryforge.

`provider-miniflux` is the always-on, multi-device counterpart to
[`provider-rss`](../provider-rss). Where `provider-rss` fetches and parses
feeds itself, `provider-miniflux` delegates to a self-hosted Miniflux server's
JSON API. Miniflux owns sync, caching, OPML import, and webhooks; Scryforge
just becomes a terminal-native client over the user's existing subscriptions.

## Capability matrix

| Trait | Implemented | Notes |
|-------|-------------|-------|
| `Provider` | yes | `health_check` hits `GET /v1/me`; `sync` is a no-op probe via `GET /v1/feeds` since the server already syncs continuously |
| `HasFeeds` | yes | `list_feeds` → `GET /v1/feeds`; `get_feed_items` → `GET /v1/entries?feed_id=...&status=...` |
| `HasSavedItems` | yes | `get_saved_items` → `GET /v1/entries?starred=true`; `save_item` / `unsave_item` use `PUT /v1/entries/<id>/bookmark` |
| `HasCollections` | no | Miniflux categories are not modelled as collections (yet) |
| `HasCommunities` | no | |

## Configuration

```rust
use provider_miniflux::{MinifluxProvider, MinifluxProviderConfig};

let config = MinifluxProviderConfig::new(
    "https://miniflux.example.com",
    "your-api-token",
);
let provider = MinifluxProvider::new(config);
```

The API token is generated in the Miniflux UI under **Settings → API Keys**.
It is sent on every request as the `X-Auth-Token` header.

### Sigilforge integration

If you store the token in [Sigilforge], enable the `sigilforge` cargo feature
and construct the config with `from_sigilforge`:

```toml
[dependencies]
provider-miniflux = { version = "0.1", features = ["sigilforge"] }
```

```rust,no_run
# #[cfg(feature = "sigilforge")]
# async fn example(fetcher: &dyn scryforge_sigilforge_client::TokenFetcher)
#     -> Result<(), Box<dyn std::error::Error>>
# {
use provider_miniflux::{MinifluxProvider, MinifluxProviderConfig};

let config = MinifluxProviderConfig::from_sigilforge(
    fetcher,
    "https://miniflux.example.com",
    "personal", // account label registered with Sigilforge
).await?;
let provider = MinifluxProvider::new(config);
# Ok(())
# }
```

The token is looked up under service `"miniflux"` and the `account` label you
pass.

[Sigilforge]: https://github.com/raibid-labs/sigilforge

## Item mapping

Miniflux entries are mapped to Scryforge items as follows:

| Miniflux Entry | Scryforge Item |
|----------------|----------------|
| `id` | `ItemId("miniflux:{id}")` |
| `title` | `title` |
| `url` | `url` |
| `content` | `ItemContent::Article.full_content` |
| `author` | `author.name` |
| `published_at` | `published` |
| `created_at` (fallback `changed_at`) | `updated` |
| `feed.title` | `metadata["feed_title"]` |
| `feed.category.title` | `metadata["feed_category"]` |
| `tags` | `tags` |
| First `image/*` enclosure (else first enclosure) | `thumbnail_url` |
| `status == "read"` | `is_read` |
| `starred` | `is_saved` |
| `feed_id` | `metadata["miniflux_feed_id"]`, `stream_id = miniflux:feed:{feed_id}` |

## Available actions

- **Open in Browser** — opens `item.url`
- **Preview** — TUI in-place preview
- **Copy Link** — copy `item.url` to clipboard
- **Mark as Read / Mark as Unread** — round-trips to `PUT /v1/entries`
- **Star Article** — round-trips to `PUT /v1/entries/<id>/bookmark`

`MarkRead`/`MarkUnread` and `Save`/`Unsave` mutate state on the Miniflux
server, so they immediately propagate to every other Miniflux client.

## Errors

`MinifluxApiError` covers the HTTP-shaped failures and is converted into
`StreamError` via `From`:

| `MinifluxApiError` | `StreamError` |
|--------------------|---------------|
| `Http(_)` | `Network` |
| `Unauthorized` / `Forbidden` | `AuthRequired` |
| `NotFound` | `Provider` |
| `RateLimited` | `RateLimited(60)` |
| `Status { .. }` / `Json(_)` / `Config(_)` | `Provider` |

## Testing

```bash
cargo test -p provider-miniflux
```

Integration tests in `tests/integration_test.rs` use [`wiremock`] to stand up
an in-process HTTP server impersonating Miniflux, so no live server is
required. They cover:

- Listing feeds
- Listing unread entries scoped to a feed
- Listing starred entries via `HasSavedItems`
- `save_item` is idempotent when the entry is already starred
- `unsave_item` toggles only when currently starred
- Mark-read action round-trips via `PUT /v1/entries`
- Star action round-trips via `PUT /v1/entries/<id>/bookmark`
- 401 responses surface as `StreamError::AuthRequired`
- `health_check` uses `GET /v1/me`

[`wiremock`]: https://crates.io/crates/wiremock

## Dependencies

- **reqwest** (0.12, `rustls-tls`, `json`) — HTTP client
- **serde / serde_json** — JSON
- **chrono** — timestamps
- **async-trait** — async trait support
- **thiserror** — error enums
- **scryforge-sigilforge-client** (optional, behind `sigilforge` feature)
- **wiremock** (dev-only) — mock Miniflux server for integration tests

## License

MIT OR Apache-2.0
