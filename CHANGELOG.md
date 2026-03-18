# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
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
