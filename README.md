<p align="center">
  <img src="logo.png" width="150" alt="logo">
</p>

# Gitlab tokens prometheus exporter

Exports the number of days before expiration of gitlab tokens as Prometheus metrics

## Getting Started

The following environment variables are **mandatory** : (locally you can use a `.env` file) :
```
GITLAB_HOSTNAME=<gitlab hostname>
GITLAB_TOKEN=<gitlab authentication token>
```

Optional environment variables :
```
DATA_REFRESH_HOURS=6 (should be > 0 and <= 24 or else, it will be set to the default value: 6)
ACCEPT_INVALID_CERTS=yes (DANGEROUS!!! disables HTTPS certificate validation when connecting to gitlab)
RUST_LOG (to configure the tracing crate)
OWNED_ENTITIES_ONLY=yes (checks only owned projects and groups - useful for gitlab.com)
```

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

When launching the exporter, it will first get infos on **all** the gitlab tokens, so it can take some time depending on the number of projects/groups/users to scan.<br />
The exporter returns `204 No Content` until the first scan is done.
