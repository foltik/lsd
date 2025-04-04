# lightandsound.design

Coming to you live.

## Setup

Install a rust toolchain with [rustup.rs](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo --version
rustc --version
```

Clone the repo:

```sh
git clone https://github.com/foltik/lsd
cd lsd
```

Create a .env file with `DATABASE_URL=sqlite://db.sqlite`

Initialize dev database:

```sh
cargo install sqlx-cli --no-default-features --features sqlite
cargo sqlx database setup
```

To automatically recompile and rerun when you make changes, use `cargo-watch`:

```sh
cargo install cargo-watch
cargo watch -x 'run config/dev.toml' --ignore "web/*"
```

Run Tailwind CLI to compile styles via `npm`.
You can optionally install the [livereload][https://www.npmjs.com/package/livereload] extension to reload the page on style changes.

```sh
npm install
# Watch for changes to ./web/styles/main.css, start livereload server
npm run watch
# Build styles minified
npm run build:styles.min
```

Use [mailtutan](https://github.com/mailtutan/mailtutan) for local testing of email functionality:

```sh
cargo install mailtutan
mailtutan
```

## Workflow

- Make commits in a separate branch, and open a PR against `main`
- When new commits land in `main`, a github action will automatically deploy the app to https://beta.lightandsound.design
