<p align="center">
  <img src="logo.png" width="150" alt="logo">
</p>

# Gitlab tokens prometheus exporter

Exports the number of days before GitLab tokens expire as Prometheus metrics.

## Configuration

The following environment variables are **mandatory** (locally you can use a `.env` file):
```
GITLAB_HOSTNAME=<gitlab hostname>
GITLAB_TOKEN=<gitlab authentication token>
```

Optional environment variables **with** defaults values:
```
DATA_REFRESH_HOURS=6 (should be > 0 and <= 24 or else, it will be set to the default value: 6)
RUST_LOG=info (to configure the tracing crate)
MAX_CONCURRENT_REQUESTS=10
MAX_RETRIES=4 (number of times a transient gitlab API error is retried; 0 disables retrying)
RETRY_BACKOFF_MS=500 (base delay for the retry exponential backoff)
SKIP_USERS_TOKENS=no
SKIP_NON_EXPIRING_TOKENS=no
SCAN_SERVICE_ACCOUNTS=no (also export the tokens of group service accounts - see "Service accounts" below)
```

Optional environment variables **not** set by default:
```
ACCEPT_INVALID_CERTS=yes (DANGEROUS!!! disables HTTPS certificate validation when connecting to gitlab)
OWNED_ENTITIES_ONLY=yes (checks only owned projects and groups - useful for gitlab.com)
USERNAMES_FILTER=jenkins,renovate-bot (comma separated list of usernames)
```

## Getting Started

Run the following commands :

if you have cargo already installed:
```
cargo build --release
```

if you want to build a OCI image with docker:
```
docker build . -t gitlab-tokens-exporter
```

## Service accounts

[Group service accounts](https://docs.gitlab.com/user/profile/service_accounts/) are a
Premium/Ultimate feature: their personal access tokens are **not** returned by the user
tokens scan and need a dedicated, Owner-scoped lookup. Setting `SCAN_SERVICE_ACCOUNTS=yes`
makes the exporter, for every top-level group it can see, list the group's service accounts
([`GET /groups/:id/service_accounts`](https://docs.gitlab.com/api/group_service_accounts/#list-service-account-users))
and then each service account's tokens
([`GET /groups/:id/service_accounts/:user_id/personal_access_tokens`](https://docs.gitlab.com/api/group_service_accounts/#list-personal-access-tokens-for-a-service-account-user)).

These tokens are exported with `type="service_account"` and a
`service_account="<group full path>/<username>"` label, for example:
```
gitlab_token_days_remaining{name="ci-token",id="123",type="service_account",service_account="my-group/my-group-ci-bot",active="true",revoked="false",scopes="[read_api]",expires_at="2026-12-13"} 30
```

Both API calls require the **Owner** role on the group. Groups where the token is not Owner
(or where the tier makes the feature unavailable) return `403`/`404` and are skipped
silently, so one inaccessible group never fails the whole scan. This feature is independent
of `SKIP_USERS_TOKENS` and does not require `is_admin`.

## Known limitations

To get the users tokens, the token used to connect to gitlab must have `is_admin`

When launching the exporter, it will first get infos on **all** the gitlab tokens (unless `OWNED_ENTITIES_ONLY` is set to `yes`), so it can take some time depending on the number of projects/groups/users to scan.<br />

The exporter returns `204 No Content` until the first scan is done.
