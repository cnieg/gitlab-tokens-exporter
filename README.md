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
SKIP_DEPLOY_TOKENS=no (skip project and group deploy tokens - see "Deploy tokens" below)
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

## Deploy tokens

[Deploy tokens](https://docs.gitlab.com/user/project/deploy_tokens/) are collected by
default (set `SKIP_DEPLOY_TOKENS=yes` to opt out). Using the same project/group traversal
as the access-token scan, the exporter calls
[`GET /projects/:id/deploy_tokens`](https://docs.gitlab.com/api/deploy_tokens/#list-project-deploy-tokens)
and [`GET /groups/:id/deploy_tokens`](https://docs.gitlab.com/api/deploy_tokens/#list-group-deploy-tokens).

Deploy tokens are exported with `type="project_deploy"` / `type="group_deploy"`. They have no
`access_level`, no `web_url` and no `active` field (their state is `revoked` plus the computed
days remaining), for example:
```
gitlab_token_days_remaining{name="registry-pull",id="42",type="project_deploy",project="group/app",revoked="false",scopes="[read_registry]",expires_at="2026-12-31"} 69
gitlab_token_days_remaining{name="ci-deploy",id="43",type="group_deploy",group="group",revoked="false",scopes="[read_repository]",expires_at="2027-01-15"} 84
```
Deploy tokens without an expiry date follow the same `SKIP_NON_EXPIRING_TOKENS` handling and
9999-days default as the other token types.

## Known limitations

To get the users tokens, the token used to connect to gitlab must have `is_admin`

When launching the exporter, it will first get infos on **all** the gitlab tokens (unless `OWNED_ENTITIES_ONLY` is set to `yes`), so it can take some time depending on the number of projects/groups/users to scan.<br />

The exporter returns `204 No Content` until the first scan is done.
