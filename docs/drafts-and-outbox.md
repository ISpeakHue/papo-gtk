# Draft recovery and outgoing messages

Drafts use the verified account identity (canonical API base including its path,
plus user UUID) and conversation UUID. Channels and DMs use the same storage
rules. A pencil in the channel/DM list marks a conversation containing a draft.
Switching conversations keeps each draft; restarting restores text, custom
embeds, reply references and available file selections.

Snapshots live beside `config.toml` under `drafts/<account namespace>.json`.
These files contain message text and attachment paths, so they are private local
data, not encrypted storage. They contain no session credentials, media bytes or
quoted reply bodies. Unix files are created with mode 0600 and new draft
directories with mode 0700; Windows uses the user's native configuration directory
and inherited account permissions. Existing atomic replacement writes preserve
the previous snapshot if replacement fails.

Disk work runs outside the GTK thread and coalesces edits with a 300 ms debounce.
Normal exit flushes the latest buffer before shutting down the async runtime. A
quick logout/login can recover the in-process buffer before its disk debounce.
An abrupt process or machine failure can still lose the most recent debounce
interval. Corrupt, unsupported, oversized or incorrectly scoped files produce a
notice and are preserved rather than overwritten with an empty store.

Each account snapshot is limited to 2 MiB, 128 conversation drafts and 20 pending
sends. Each draft permits 65,536 text characters, the existing validated embed
limits and up to ten attachment references. Exceeding storage limits reports a
save error; it does not erase the previous saved file. Draft text is allowed to
exceed the backend's send limit so unfinished text is not lost simply because it
cannot yet be submitted.

Files are inspected again during recovery. Missing files, changed sizes,
relative paths and temporary clipboard selections are omitted with a persistent
warning to select them again. No attachment bytes are copied into the store.
Reply IDs are fetched for the selected conversation before sending; a confirmed
deleted target clears the reference and preserves the text. An unavailable
server keeps the reference and offers retry or removal. Live deletions also
clear affected draft references; queued replies fail for explicit review.

## Sending

Submission validates the payload, moves it to a bounded account-owned queue,
and immediately clears and refocuses the composer. One send runs at a time;
switching channels does not cancel it. New typing is independent of completion.
Pending rows show queued, sending/upload progress or failed status. Successful
responses use the normal canonical message row; message-ID upserts also handle
the corresponding live event without duplicating canonical messages.

A failed row retains text, embeds and file selections, with retry, edit and
discard actions. Editing requires an empty composer so it cannot overwrite a
newer draft. Active cancellation aborts the local request and keeps an uncertain
failed row: it cannot recall a request already accepted by the server. Permission
changes are checked before starting queued messages. Recovered sends always start
as failed/review-required, even if they were queued before shutdown. Every
recovered send is treated as uncertain: a debounced snapshot cannot prove that
a later request was never attempted.

The backend has **no idempotency contract for ordinary user message POSTs**.
Papo therefore never automatically retries them. Network errors, timeouts, server
errors and interrupted active sends require confirmation before manual retry;
the dialog asks the user to check history for a possible successful delivery.
Definite client rejection can be retried explicitly without that extra dialog.
No deduplication by message text is attempted. Webhook idempotency does not make
ordinary sends safe to replay. See [backend contracts](backend/ux-contracts.md).

## Regression checks

Pure Rust tests cover scope validation, corruption/limits, debounce/exit flush,
quick relogin, independent queue ordering, stale canceled callbacks, revoked
permissions and pending file/embed recovery. GTK cases `draft-recovery` and
`outgoing-queue` exercise restart restoration, missing files/deleted replies,
editable composition during delayed sends, channel switching and uncertain
retry confirmation. `multiline` verifies that editing a failed send retains
newlines and mention UUIDs. Linux and Windows CI include these cases; native
Windows results must be obtained from CI or a Windows desktop.
