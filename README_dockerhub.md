# Gitlab tokens prometheus exporter

Exports the number of days before expiration of gitlab tokens as Prometheus metrics

## Getting Started

The following environment variables are mandatory :

```
GITLAB_BASEURL=<gitlab hostname>
GITLAB_TOKEN=<gitlab authentication token>
```

Optional environment variables :
```
DATA_REFRESH_HOURS=6 (should be > 0 and <= 24 or else, it will be set to the default value: 6)
```

You can launch an instance using the following docker command :
```
docker run -it --rm -e "GITLAB_BASEURL=__hostname__" -e "GITLAB_TOKEN=__token__" cnieg/gitlab-tokens-exporter:latest
```

## Known limitations

When launching the exporter, it will first get infos on **all** the gitlab tokens, so it can take some time depending on the number of projects to scan.

The exporter returns `204 No Content` until the first scan is done.