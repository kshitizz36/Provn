# Releasing Provn

## One-time setup

- GitHub repo Settings > Actions > General > Workflow permissions > **Read and write permissions**
- Add repository secret `NPM_TOKEN` to publish `@kshitizz36/provn` to npm
- Optional: create `kshitizz36/homebrew-tap` and add repository secret `TAP_GITHUB_TOKEN`

## Release flow

1. Bump versions in `provn-cli/Cargo.toml` and `npm/package.json`
2. Run:
   - `cd provn-cli && cargo test`
   - `cd provn-cli && cargo clippy -- -D warnings`
3. Commit changes
4. Tag and push:
   - `git tag -a vX.Y.Z -m "Provn vX.Y.Z"`
   - `git push origin main`
   - `git push origin vX.Y.Z`

## What the workflows do

- `release.yml` builds platform binaries and creates a GitHub Release
- `release.yml` publishes the npm package when `NPM_TOKEN` is configured
- `update-homebrew.yml` updates the Homebrew tap when `TAP_GITHUB_TOKEN` is configured
