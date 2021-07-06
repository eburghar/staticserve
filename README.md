# staticserve

A simple and fast async http(s) server for hosting static website (JAMSTACK) under kubernetes, and replacing nginx.
- `staticserve` defines an API point protected by a token to upload and decompress a new site archive (.tar or
  .tar.zst) that can be easily used in CI/CD pipelines,
- allows to define dynamic routes to accomodate bookmarked links (vuerouter for example).

## Usage

```
Usage: staticserve [-c <config>] [-v] [-a <addr>]

Extract latest projects archives from a gitlab server

Options:
  -c, --config      configuration file containing projects and gitlab connection
                    parameters
  -v, --verbose     more detailed output
  -a, --addr        addr:port to bind to
  --help            display usage information
```

## Configuration

Create a `/etc/staticserve.yaml`

```yaml
dir: /var/lib/staticserve
root: dist
tls: true
crt: /var/run/secrets/staticserve/tls.crt
key: /var/run/secrets/staticserve/tls.key
token: xxxxxxxxxxxxxx
routes:
  "/search/category/{category}/{search}": "search/category/_category/_search.html"
```

The server support transparent compression and add a `max-age=3600` on all served files.

It uses the [fast rustls](https://jbp.io/2019/07/01/rustls-vs-openssl-performance.html) tls implementation.

## Uploading

```sh
curl -H 'token: xxxxxxxxxxxxxx' -F file=@blog.tar.zst https://host/upload
```

The server will restart automatically to serve the new content.
