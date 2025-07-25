# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.5](https://github.com/gwen-lg/subtile-ocr/compare/v0.2.4...v0.2.5) - 2025-07-24

### Added

- *(vobsub)* manage `sub` file without associated `idx`
- *(taplo)* add Taplo configuration file

### Fixed

- migrate to use palette vobsub conversion

### Other

- update dependencies
- enable additionnal clippy lints in Cargo.toml
- *(cargo)* enable reorder_keys formating for lints
- improve rust cache use
- improve subtitle extension management
- remove useless pix type define for passthrough
- *(subtile)* update to subtile v0.4 dependency
- *(release-plz)* use publishing environment
- *(commits)* disable fast fail on matrix strategy
- *(release-plz)* update workflow from v0.3.138
- *(error)* remove capital letter at start of error message
- *(commits)* add check-commits on push
- *(commits)* add new workflow to check commits
- *(checks)* dont build doc for deps
- *(checks)* add check of Cargo.toml formating with Taplo
- *(checks)* create checks action with repository checks
- ignore `wip*` branch on push
- *(release-plz)* update release-plz action
- add dependabot.yml for github
- *(clippy)* pass profiling_data argument by ref to write_perf_file fn
- *(clippy)* enable addidionnal clippy lints
- add missing const on some fn
- add conf with allowed-duplicate-crates
- add FUNDING.yml file with liberapay account
# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4](https://github.com/gwen-lg/subtile-ocr/compare/v0.2.3...v0.2.4) - 2025-02-08

### Added

- *(ocr)* export ocr::process symbol

### Fixed

- *(ocr)* also init `TESSERACT` on main thread

### Other

- *(ocr)* error for already initialized TESSERACT
- *(ocr)* manage Tesseract init error
- *(code_check)* add `--locked` to cargo steps
- *(code_check)* remove `exit 1` in error summary check
- *(ocr)* Add documentation to ocr::Error

## [0.2.3](https://github.com/gwen-lg/subtile-ocr/compare/v0.2.2...v0.2.3) - 2025-01-07

### Other

- *(release-plz)* configure Release-plz for pr + release

## [0.2.2](https://github.com/gwen-lg/subtile-ocr/compare/v0.2.1...v0.2.2) - 2025-01-07

### Added

- *(puffin)* use a BufWriter for pref stats file

### Other

- *(gitignore)* remove data files of ignore
- *(release-plz)* configure Release-plz for pr
- *(cargo)* fix alphabetical order of dependencies
- *(cargo)* update subtile dependendies to 0.3.1
- fix github workflows change for trigger code_check
- *(cargo)* update dependendies

## [0.2.1](https://github.com/gwen-lg/subtile-ocr/compare/v0.2.0...v0.2.1) - 2024-08-11

### 🚀 Features

- *(typos)* Add .typos.toml conf
- *(dump)* Add DumpImage error and map to call to dump_images
- *(error)* Create a dedicated error for Index file opening
- *(error)* Create an error for RayonThreaPool creation
- Run cargo update
- *(subtile)* Update subtile dependency to v0.3"
- *(pgs)* Add pgs (*.sup files) management
- Add dump-raw option

### 🚜 Refactor

- Rework ocr thread management
- `run` fn return a simple Result
- Convert ocr process to use IntoParrallelTterator
- Inline code of process_image_for_ocr

### 📚 Documentation

- Change `PNGs` to `PNG files`
- *(error)* Add documentation for errors of check_subtitles
- *(cargo)* Add 'pgs' as keyword for crate

### 🎨 Styling

- Update to multiline `use`
- Inline format args
- *(cargo)* Move dependencies before lints setup

### ⚙️ Miscellaneous Tasks

- *(typos)* Add typos step in code_check ci workflow
- *(github)* Update runs-on to `ubuntu-latest`

## [0.2.0] - 2024-07-18

### 🚀 Features

- *(ocr)* Blacklist `[]` character in addition to `|`
- Display error stack in check_subtitles warning
- Convert check_subtitles to use IntoIterator
- *(migrate)* Rework parsing use and subtitle management

### ⚙️ Miscellaneous Tasks

- Cargo update
- [**breaking**] Migrate to subtitle v0.2.0
- *(migrate)* To subtile 0.2.0: rename SubError to SubtileError
- Release v0.2.0

### Refacto

- *(preprocess)* Remove now useless ImagePreprocessOpt struct
- *(preprocess)* Rename preprocess_subtitles in process_images_for_ocr
- *(preprocess)* Add generic for images parameter in process_images_for_ocr

## [0.1.8] - 2024-07-12

### Github-ci

- Now use ubuntu-stable and checkout@v4

### Ocr

- Avoid tesseract tried to invert image

## [0.1.7] - 2024-06-04

### Clean

- Remove useless DumpImage error entry

## [0.1.6] - 2024-05-20

### Clean

- Remove useless empty line

## [0.1.5] - 2024-05-19

### Leptess

- Blacklist pipe character

## [0.1.4] - 2024-04-09

### Cargo

- Update dependencies + image to v0.25
- Remove patch version specification for some dependencies
- Configure clippy lints by categories

### Check

- Add missing docs and enable lint

### Clippy

- Enable various lint from pedantic
- Cast_lossless fixes + enable lint
- Add must_use and enable lint
- Fix uninlined_format_args and enable lint

### Gitignore

- Ignore *.orig file generated by git

### V0.1.4

- Publish a new version

## [0.1.3] - 2024-02-04

### ⚙️ Miscellaneous Tasks

- Rename workflow & cleanup names
- Manage features in github build action

### Cargo

- Update subtile to v0.1.1 and other dependencies

### Profiling

- Add profile-with-puffin feature
- Add puffin profiling on a feature
- Include date/time in capture filename.

### Readme

- Updated to reflect the fork and name change

### V0.1.3

- Publish a new version

## [0.1.2] - 2024-02-03

### ⚙️ Miscellaneous Tasks

- Add build github action copied from subtile

### Cargo

- Update Snafu dependency from 0.6.10 to 0.7
- Update image from 0.23.14 to 0.24
- Update clap from 3 to 4.2
- Update package informations to reflect the fork
- Update dependencies
- Update package edition to 2021
- Update version to 0.1.2

### Clap

- Update derive use from deprecated check

### Clean

- Remove useless Result redefinition
- Add an error context to run call
- Create and use a struct ImagePreprocessOpt

### Clippy

- Remove use vobsub;
- Remove Ok( + ?)
- Remove useless call to clone
- Remove call to into_iter on y_range
- Remove useless result var
- Change .len() == 0 into .is_empty() in if
- Remove useless & in call to subtitle_to_images
- Remove cast of offset as usize

### To_lib

- Rename main.rs into lib.rs
- Add pub export
