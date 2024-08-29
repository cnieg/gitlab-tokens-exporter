<p align="center">
  <img src="logo.png" width="150" alt="logo">
</p>

# Gitlab tokens prometheus exporter

Exports the number of days before expiration of gitlab tokens as Prometheus metrics

## Getting Started

Provide the following environment variables (locally you can use a `.env` file) :

```
GITLAB_BASEURL=<gitlab hostname>
GITLAB_TOKEN=<gitlab authentication token>
DATA_REFRESH_HOURS=6 (should be > 0 and <= 24 or else, it will be set to the default value: 6)
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

When launching the exporter, it will first get infos on **all** the gitlab tokens, so it can take some time depending on the number of projects to scan.
The exporter returns `204 No Content` until the first scan is done.