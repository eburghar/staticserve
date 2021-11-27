# staticserve

A simple and fast async http(s) server for hosting static website (JAMSTACK) under kubernetes. `staticserve`
defines an API point (`/upload`) optionally protected by a JWT token to update website content by uploading an
archive (.tar or .tar.zst).

A JWKS endpoint is used to retrieve the public keys needed to verify the validity of the token, and a claims map
with expected values can be used to restrict even more who can upload new content to the server. This is specially
useful in CI/CD pipelines.

You can also define dynamic routes to accomodate bookmarked links (vuerouter for instance), and cache control
header based on path prefix and suffix.

The server support transparent compression and add cache-control header based on path prefixes and suffixes. It
supports SPA applications by serving files based on path expressions via the routes dictionnary.

It uses the [fast rustls](https://jbp.io/2019/07/01/rustls-vs-openssl-performance.html) tls implementation.

It helps me to get 100% at pagespeed.web.dev for my svelte based blog.

## Usage

```
Usage: staticserve [-c <config>] [-v] [-a <addr>]

Static file server with ability to upload content and define dynamic routes

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
tls:
  crt: /var/run/secrets/staticserve/tls.crt
  key: /var/run/secrets/staticserve/tls.key
jwk: https://gitlab.com/-/jwks
# only allow a job from a particular project running on a protected branch or tag to update content
claims:
  iss: gitlab.com
  project_path: node/blog
  ref_protected: true
routes:
  "/search/category/{category}/{search}": "search/category/_category/_search.html"
cache:
  prefixes:
    '/app_/': 'max-age=259200'
  suffixes:
    'index.html': 'private,max-age=0'
``

## Uploading

```sh
curl -H 'Authorization: Bearer: $CI_JOB_JWT' -F file=@blog.tar.zst https://host/upload
```
