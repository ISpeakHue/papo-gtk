# Fixes from bug_list.txt

| Item | Change | Regression coverage |
| --- | --- | --- |
| 1. Unexpected chat scrolling | Preserve unchanged rows and the visible message's offset. Restore scrolling after allocation; navigate only on an explicit request. New messages follow the bottom only when already there. | Mapped chat checks typing, drafts, pin events, live arrivals, pagination, programmatic focus and keyboard focus. |
| 2. Role nickname colors | Apply validated role colors to chat authors, members and profile labels using Pango attributes. The first valid assigned color follows the backend's role order. | Color validation tests and GTK role-label checks. |
| 3. Image previews | Bound inline thumbnails to 420 × 300 pixels, preserve aspect ratio and stop stretching cards across the chat. Clicking opens an image window. | Mapped image-width/aspect checks, viewer opening and moderation cleanup. |
| 4. Reply button | Preserve button widgets during unrelated updates; keep the reply draft and focus the composer. | Actual reply-button activation, timer updates, cancellation and send-permission checks. |
| 5. Notifications button | Move notifications and their unread badge into the conversation toolbar beside pins and members. | Existing inbox workflow and mapped toolbar previews. |
| 6. Notification destination | Resolve notification metadata before navigating; show and highlight the target after history allocation. Retain channel permission checks. | Actual notification-button activation and visible-target assertion. |
| 7. Older-message banner | Show “Você está vendo mensagens antigas” above the composer with a “Mais recentes” button. | Scroll-away, live-arrival, jump-to-bottom and banner-state checks. |
| 8. Text channel selection | Handle the native list's row-activated signal instead of the row widget's activation signal. | Activate actual channel rows and verify the selected channel changes. |
| 9. Voice channel selection | Use native row activation to open the voice-room panel. | Activate a voice row and check panel visibility, join, signaling and cleanup with the mock voice engine. |
| 10. Search layout | Keep the query and results prominent; place labeled advanced filters in a collapsed expander. | Existing search failure/retry/paging/navigation checks plus initial filter-state checks and a preview. |
| 11. Profile layout | Enlarge the window, group avatar/name/roles, constrain the description field and wrap image controls. | Existing profile-write checks plus initial visible-control checks and a preview. |
| 12. Session accumulation | Save a valid authentication cookie in the desktop Secret Service and resume through whoami and refresh when reopening. Keep remembered cookies in sync with refresh rotation. Explicit logout removes the saved credential; expired sessions are omitted from the sessions list. | Local HTTP tests require whoami/refresh instead of login; check scope, rotation, revocation, deletion and credential redaction. A separate disposable-secret test exercises the real keyring. |
| 13. Message context menu | Right-click, touch-and-hold, Menu or Shift+F10 opens message actions. Add reply and copy text alongside permission-controlled edit/delete/pin/reaction actions. | Mapped menu, real clipboard, reply and permission checks. |

Session reuse requires “Manter sessão neste dispositivo” and an available desktop
keyring. An expired/revoked session requires authentication again. Explicit new
logins still create sessions according to backend policy. Existing valid sessions
remain available for manual revocation in account security.

The tests use local mock APIs and synthetic media. They do not log into a
production server or establish a production voice call. See [TESTING.md](TESTING.md)
for commands and the separate native media integration checks.

Verified locally on 2026-10-07: 119 regular tests passed; the combined GTK
regression workflow passed; the real desktop keyring round trip passed; and
`cargo build --locked --offline` succeeded. Search, profile, inline-image and
history-banner previews were inspected using software rendering.
