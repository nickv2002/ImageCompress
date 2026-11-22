export PATH := /opt/homebrew/opt/rustup/bin:$(PATH)
SHELL := /bin/bash
SCHEME := ImageCompress
DERIVED := build/DerivedData
XB := xcodebuild -project macos/ImageCompress.xcodeproj -scheme $(SCHEME) -derivedDataPath $(DERIVED)

.PHONY: generate cli build test run licenses linux source-tarball universal check-universal parity release release-quiet

generate:
	cd macos && xcodegen generate

cli:
	cargo build --release

build: generate cli
	$(XB) -configuration Debug build

test: generate cli
	cargo test
	$(XB) -configuration Debug test

run: build
	open $(DERIVED)/Build/Products/Debug/ImageCompress.app

licenses:
	scripts/collect_notices.sh

universal:
	scripts/build_cli_mac.sh
	scripts/build_gifsicle_mac.sh

check-universal: universal
	scripts/check_cli_mac.sh

linux:
	scripts/build_linux.sh

source-tarball:
	scripts/make_source_tarball.sh

parity:
	scripts/parity.py

# Full release (see .claude/skills/release-imagecompress/SKILL.md).
release:
	scripts/release.sh

# The same, printing only the ==> progress lines; the full output goes to build/release.log.
release-quiet:
	@mkdir -p build
	@set -o pipefail; scripts/release.sh 2>&1 | tee build/release.log | grep --line-buffered -E '^(==>|error|warning|    warning)' || { echo "release failed; tail of build/release.log:"; tail -20 build/release.log; exit 1; }
