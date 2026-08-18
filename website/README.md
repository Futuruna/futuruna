# Futuruna website

The website uses Dioxus Fullstack static-site generation. The build briefly runs
the native renderer, writes complete HTML for every canonical route, and ships
the WebAssembly client alongside it for hydration and interactive features.
No Rust server runs in production.

## Build and verify

Install `dioxus-cli` 0.7.10 and the WebAssembly target, then run from the
repository root:

```sh
./website/scripts/build-ssg.sh
```

The verified Cloudflare Pages artifact is written to:

```text
website/target/dx/futuruna-web/release/web/public
```

The wrapper intentionally uses
`dx build --web --ssg --release --locked --force-sequential --debug-symbols false`.
Sequential mode ensures the browser bundle and its `index.html` exist before the
native renderer starts. Release debug symbols are disabled so `wasm-opt` can
produce the small production module. Dioxus 0.7.10 accepts `--ssg` on
`dx bundle`, but that command does not invoke the prerender step.

## Deploy

Preview a non-production branch:

```sh
./website/scripts/deploy-pages.sh my-preview-branch
```

After the commit is on `main`, deploy production with:

```sh
./website/scripts/deploy-pages.sh main
```

The production custom domain belongs to the Cloudflare Pages project
`futuruna`. The deploy wrapper pins Wrangler 4.123.0 and verifies that a
production commit matches the freshly fetched `origin/main`. Do not deploy this
artifact to the older `futuruna-deploy` project.

## Canonical host redirect

The `futuruna-www-redirect` Worker owns the `www.futuruna.com` custom domain and
permanently redirects every request to `https://futuruna.com`, preserving the
path and query string. Cloudflare manages the custom domain's DNS record and TLS
certificate. Do not also attach `www.futuruna.com` to the Pages project.

Test and deploy the Worker from the repository root:

```sh
node --test website/cloudflare/www-redirect/worker.test.mjs
npx --yes wrangler@4.123.0 deploy \
  --config website/cloudflare/www-redirect/wrangler.jsonc
```

To roll back, remove the Worker's `www.futuruna.com` custom domain, delete the
Worker, and confirm that `www` no longer resolves. Cloudflare can retain the
automatically issued certificate after custom-domain removal; remove that
certificate separately from SSL/TLS settings if it is no longer needed.
