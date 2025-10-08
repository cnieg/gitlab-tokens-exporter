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
SKIP_USERS_TOKENS=no
```

Optional environment variables **not** set by default:
```
ACCEPT_INVALID_CERTS=yes (DANGEROUS!!! disables HTTPS certificate validation when connecting to gitlab)
OWNED_ENTITIES_ONLY=yes (checks only owned projects and groups - useful for gitlab.com)
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

## Known limitations

To get the users tokens, the token used to connect to gitlab must have `is_admin`

When launching the exporter, it will first get infos on **all** the gitlab tokens (unless `OWNED_ENTITIES_ONLY` is set to `yes`), so it can take some time depending on the number of projects/groups/users to scan.<br />

The exporter returns `204 No Content` until the first scan is done.
