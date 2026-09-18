# Accounts check

`GET /backend-api/wham/accounts/check` · Alias: `/api/codex/accounts/check`

**Request:** No body.

**Response:** `{accounts:AccountEntry[] or object keyed by account ID,account_ordering:[string],default_account_id?:string}`.

`accounts` may be a list of `{id,name?,profile_picture_url?,structure}` or a map whose values contain `{account:{account_id?,name?,profile_picture_url?,structure}}`. The gateway preserves the upstream representation.

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/backend-client/src/client.rs#L344) · [Native rules](native.md)
