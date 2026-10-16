# Webhook support

The frontend follows papo-backend revision `b2f2c43` (including the persistent
webhook identity added in `4179747`). No backend changes are required.

Open the server-name menu and choose **Webhooks**. The server owner and members
with the independent **Gerenciar webhooks** role permission can:

- List active webhooks and create one for a text or voice channel. Categories
  and direct messages are excluded.
- Copy the sending endpoint and the token returned at creation. The token is
  masked, appears only in the current dialog, and is cleared when the dialog
  closes, the webhook is revoked, or management access is lost. It is not saved
  to configuration or session storage and is redacted in debug output.
- Edit the persistent nickname and choose/remove an avatar. Empty nicknames
  restore the webhook's original name. Avatars use the backend's existing
  2 MiB / 512 × 512 limit and PNG, JPEG, GIF or WebP formats.
- Revoke a webhook after confirmation. Existing messages keep their attribution.

The integration sends requests to the copied endpoint with
`Authorization: Bearer <token>`, using the backend's multipart message contract.
The desktop user's login cookie is used only for management; it cannot execute
webhooks. Papo GTK does not send test messages or store integration tokens.

## Incoming messages

REST history, pinned messages, and live `message` events retain `webhook_id` and
`external_author`. Chat displays the external sender's name, avatar and an APP
badge with the source in its tooltip. Replies and pins use the same sender name;
webhook identities never become Papo accounts or gain ownership permissions.
The normal live edit/delete/embed/moderation events continue to update these
messages, with no webhook polling or extra history refreshes.

Small avatars are fetched only for relevant messages, decoded off the GTK thread,
and cached across channel changes (256 entries, at most two requests at once).
Distinct URLs enter a bounded queue instead of being dropped when slots are
full. Repeated occurrences share one download and texture. Discord CDN aliases,
size options and expiring query signatures share a cache entry for the same
avatar resource; requests retain the original signed URL. The backend does not
provide an external-avatar batch endpoint, so the client coalesces a viewport's
work into this queue rather than issuing a request per message.

Channel switches discard obsolete queued work while allowing the two active
small downloads to finish into the session cache. Failed loads are cached
briefly; a failed refresh preserves an existing picture. Changing an avatar in the manager
invalidates its local cache. Public avatar requests use a separate HTTP client
without Papo cookies or Authorization, reject redirects, and enforce a 2 MiB
response limit. Requests to the Papo server share the session request budget;
external CDN requests share a separate budget of at most two starts per second.
Both paths use bounded retries and honor `Retry-After` on rate limits.

Consecutive webhook messages group using the webhook ID, external source and
external sender ID, with the existing five-minute/date/reply/pin/unread boundaries.
Different people relayed by the same webhook never merge. External names and
pictures retain normal contrast; noninteractive profile controls cannot receive
clicks or keyboard focus instead of inheriting GTK's disabled appearance.

The current backend's `ExternalAuthor` contains only `source`, `id`, `name`,
`avatar_url` and `profile_url`: no role or display color is sent. External names
therefore use the theme's normal text color. Native Papo users still use their
role colors. Supporting external role colors requires that metadata in the
backend contract; matching external names to native accounts would be ambiguous.

The backend already supplies external sender names in search results and channel
previews. Notifications use the sender metadata when their message is known;
the notification REST/event contracts do not independently include that metadata.
Audit entries with `metadata.webhook_id` identify the machine rather than
attributing its actions to the credential's creator. Legacy bot embeds continue
to render through their existing path.
