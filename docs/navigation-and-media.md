# Conversation navigation and media controls

## Quick conversation switcher

`Ctrl+K` opens **Alternar conversa**; `Ctrl+F` opens message search. The
switcher searches the current account's readable text/voice-chat channels and
existing direct conversations locally, without fetching message history to
produce suggestions. Search matches conversation names, usernames and the
server name, ignoring case and an initial `#` or `@`.

Exact names and name prefixes rank before other matches. Within each group,
unread conversations come first, followed by recently visited conversations and
alphabetical names. The session remembers up to 50 recent conversations and
shows at most 100 results. This is navigation within the current server/account;
saved multi-server switching is a later plan step.

Use Up/Down to select a result while keeping the search field focused, Enter
to open it, and Escape to close. Closing restores the previous focused control
when available, otherwise the composer. Choosing a conversation focuses the
composer and retains the previous conversation's draft. Results refresh as
channel access, direct conversations or socket state change; choosing a stale
channel rechecks current access before navigating.

## Media feedback and retries

Each image/GIF placeholder distinguishes **Baixando mídia…**, **Processando
mídia…**, manual loading and failure. Moderation-pending, processing and blocked
attachments keep their own explanation. Sensitive media still requires explicit
reveal; loading preferences do not bypass moderation or channel access.

Placeholders use the available media dimensions and the existing bounded chat
frame while bytes download/decode. Unknown dimensions use the existing fallback
frame. A **Tentar novamente** action retries the affected item without reloading
the conversation. Video-download failures use the same local feedback and retry
their original media, rather than only their thumbnail. If loading a viewer's
original image fails, its usable preview remains and an original-image retry is
offered inside the viewer.

Nonvisual file tiles show download progress/failure beside the file, disable
duplicate activation during the download, and allow another attempt by clicking
the failed tile. A successful-save label disappears after three seconds. Session
errors still reach the existing account/session handler.

## Device preferences

Open **Configurações da conta → Preferências → Mídia e armazenamento (neste
dispositivo)**. These choices are stored in local configuration, never in the
backend's account settings payload:

- **Automático** retains bounded background admission for the opening message
  batch and visible media.
- **Ao clicar** starts new image/GIF requests only after an explicit load or
  reveal action. Switching to it cancels unrequested background preview work;
  already loaded previews remain usable. Explicit original-image/video actions
  continue to work. Avatars and custom emoji use their separate small-asset caches.
- **Reduzir uso de memória** lowers retention and animation-frame budgets. It
  does not change the saved GIF/emoji motion preference. A GIF can fall back to
  its first frame when its decoded frames exceed the selected budget.

| Chat cache | Normal | Low memory |
| --- | ---: | ---: |
| Active decoded previews | 64 entries / 64 MiB | 16 entries / 16 MiB |
| Resident recent previews | 256 entries / 96 MiB, eight channels | 256 entries / 16 MiB, two channels |
| Compressed previews/renditions | 512 entries / 64 MiB, eight channels | 512 entries / 8 MiB, two channels |
| Decoded animation frames | 48 MiB | 8 MiB |
| Cached temporary video files | 128 MiB | 16 MiB |

These are separate ceilings, not a total process-memory guarantee. Older
nonprotected resident/compressed history also has smaller sub-budgets and idle
expiry. Low-memory eviction avoids repeatedly fetching the same visible item:
an evicted preview can be loaded explicitly, or considered again after changing
conversation. Oversized compressed entries are rejected without flushing useful
smaller entries.

## Usage and clearing

**Cache de mídia do chat** reports accounted in-memory preview textures,
animation frames, compressed bytes and staged image payloads, plus cached
temporary video file sizes. Shared active/resident texture objects are counted
once. Refresh the row to sample again.

The figures exclude total process RSS, GTK/GPU renderer allocations, avatars and
emoji caches, other dialog caches, open original-image viewers, in-flight
allocations and files retained solely by active players. They are not a live
measurement of all application storage or memory.

**Limpar cache de mídia** releases those caches and cached chat surfaces, also
clearing an open pinned-message view. It cancels background media work and
invalidates its pending completions. Open viewers/players, uploads and explicit
file saves retain their own resources and continue. Display geometry and
message metadata remain available. The current view waits for explicit loads
instead of immediately refilling its cache; selecting a conversation resumes the
chosen automatic/on-click policy. Closing owners can release additional native
resources later, so RSS need not fall immediately.

Step 10 now adds a bounded, account-scoped disk thumbnail cache. Usage includes
its separate disk total, and clearing also removes these thumbnails. See
[disk thumbnails, media browsing and call setup](media-browser-and-call-setup.md)
for the limits, current backend coverage and device tests.
