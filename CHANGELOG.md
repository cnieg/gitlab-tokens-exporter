# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
## [3.3.0] - 2026-07-23

### 🚀 Features

- Filter user tokens by username
- *(env)* Override any existing environment variables of the same name

### 💼 Other

- *(github)* Use sha1 instead of tags for github actions
- *(Dockerfile)* Use digests

### ⚡ Performance

- Remove clone call
- Move regex definition in CONFIG (to instantiate once)

### 🎨 Styling

- *(clippy)* Fix inline_trait_bounds lint

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust docker tag to v1.97.0
- *(deps)* Update rust crate tokio to v1.52.4
- *(deps)* Update rust crate regex to v1.13.1
- *(deps)* Update rust crate serde_repr to v0.1.21
- *(deps)* Update rust crate serde to v1.0.229
- *(deps)* Update rust crate anyhow to v1.0.104
- *(deps)* Update rust crate tokio to v1.53.0
- *(deps)* Update rust crate tokio to v1.53.1
- *(deps)* Update rust crate serde_json to v1.0.151
- Rename variable
## [3.2.0] - 2026-07-04

### 🚀 Features

- Retry transient gitlab API errors with exponential backoff

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate anyhow to v1.0.103
- *(deps)* Update actions/checkout action to v7
## [3.1.0] - 2026-06-17

### 🚀 Features

- Use anyhow to add context to all errors

### 🚜 Refactor

- Use a `Config` struct
- Configuration globally available via the static variable `CONFIG`
- *(deserialize_optional_date)* Easier to read
- *(state_actor)* Reduce code duplication using new traits
- *(config)* New `get_bool_or_false()` function

### 📚 Documentation

- Fix metric name and labels

### ⚡ Performance

- Remove a call to `clone()`

### 🎨 Styling

- Log output == actual output

### ⚙️ Miscellaneous Tasks

- Fix comments
- *(deps)* Update rust docker tag to v1.94.1
- *(deps)* Update rust crate tokio to v1.51.0
- *(config)* Migrate config renovate.json
- *(deps)* Update rust crate tokio to v1.51.1
- Remove call to deprecated function `danger_accept_invalid_certs()`
- *(deps)* Update rust crate parse_link_header to v0.4.1
- *(deps)* Update rust crate axum to v0.8.9
- *(deps)* Update rust crate tokio to v1.52.1
- *(deps)* Update alpine docker tag to v3.23.4
- *(deps)* Update rust docker tag to v1.95.0
- *(deps)* Update rust crate reqwest to v0.13.3
- *(deps)* Update rust crate tokio to v1.52.2
- *(deps)* Update rust crate tokio to v1.52.3
- *(deps)* Update rust crate serde_json to v1.0.150
- *(deps)* Update rust crate reqwest to v0.13.4
- *(deps)* Update rust crate chrono to v0.4.45
- *(deps)* Update rust crate regex to v1.12.4
- *(deps)* Update alpine docker tag to v3.24.0
- *(deps)* Update rust docker tag to v1.96.0
- *(deps)* Update alpine docker tag to v3.24.1
- *(Dockerfile)* Use `&&` instead of `;` when chaining multiple commands
- *(musl-cross)* Check sha256sum value
- *(musl-cross)* Update to 20260515
- Remove unnecessary dev dependency
- Remove useless conversion
## [3.0.2] - 2026-03-18

### 🎨 Styling

- Replace tab with spaces

### ⚙️ Miscellaneous Tasks

- *(deps)* Update docker/login-action action to v4
- *(deps)* Update rust crate tracing-subscriber to v0.3.23
- *(deps)* Update rust crate once_cell to v1.21.4
- *(deps)* Update docker/setup-buildx-action action to v4
- *(deps)* Update docker/build-push-action action to v7
- *(deps)* Update rust docker tag to v1.94.0
- Optimize for size
## [3.0.1] - 2026-03-03

### 🐛 Bug Fixes

- *(Dockerfile)* Fix directories access rights

### ⚙️ Miscellaneous Tasks

- *(deps)* Update rust crate tokio to v1.50.0
## [3.0.0] - 2026-02-27

### 🚀 Features

- *(metrics)* [**breaking**] Unify token metrics name

### ⚙️ Miscellaneous Tasks

- More idiomatic syntax
- Use axum .with_graceful_shutdown()
- Refactor `use` declarations
- Smaller OCI image
- *(Dockerfile)* Use POSIX sh syntax
- *(deps)* Update rust crate chrono to v0.4.44
- Disallow the `ref_patterns` lint
## [2.5.7] - 2026-02-13

### 🎨 Styling

- *(clippy)* Allow `doc_paragraphs_missing_punctuation` lint

### ⚙️ Miscellaneous Tasks

- Simplify PR pipeline
- Use release-plz
- *(deps)* Update rust crate reqwest to v0.13.2
- *(deps)* Update rust docker tag to v1.93.1
- *(deps)* Update rust crate regex to v1.12.3
