# Plugin read

`GET /backend-api/ps/plugins/{plugin_id}`

**Request:** Query includeDownloadUrls?.

**Headers:** `oai-product-sku`.

**Response:** JSON `{id: string, name: string, scope: "GLOBAL" | "USER" | "WORKSPACE", discoverability?: "LISTED" | "UNLISTED" | "PRIVATE", creator_account_user_id?: string, creator_name?: string, share_url?: string, share_principals?: RemotePluginDirectorySharePrincipal[], can_publish_to_workspace?: boolean, installation_policy: PluginInstallPolicy, installation_policy_source?: RemotePluginInstallPolicySource, must_show_installation_interstitial?: boolean, authentication_policy: PluginAuthPolicy, status?: PluginAvailability, disabled_reason?: PluginDisabledReason, eligible_plan_types?: string[], release: RemotePluginReleaseResponse}`. Nested fields: [RemotePluginDirectoryItem](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L682).

`release` includes `display_name`, `description`, `version?`, `bundle_download_url?`, `app_ids`, `app_manifest?`, `app_templates`, `keywords`, `interface`, `skills`, `mcp_servers`, `scheduled_tasks?`. Bundle URLs may be rewritten to [transfers](transfers.md).

[Pinned source](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core-plugins/src/remote.rs#L2263) · [Native rules](native.md)
