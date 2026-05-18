.PHONY: all build build-derive test test-figtree test-derive clean patch minor major

# ── Workspace targets ─────────────────────────────────────────────────────────

# Default — build everything in the workspace
all: build

# Build both crates in release mode
build:
	cargo build --release --package figtree
	cargo build --release --package figtree-derive
	@echo "✅ Built figtree and figtree-derive"

# Build figtree only
build-figtree:
	cargo build --release --package figtree
	@echo "✅ Built figtree"

# Build figtree-derive only
build-derive:
	cargo build --release --package figtree-derive
	@echo "✅ Built figtree-derive"

# ── Test targets ──────────────────────────────────────────────────────────────

test:
	cargo test --quiet --workspace
	@echo "✅ All tests passed"

test-figtree:
	cargo test --quiet --package figtree
	@echo "✅ figtree tests passed"

test-derive:
	cargo test --quiet --package figtree-derive
	@echo "✅ figtree-derive tests passed"

# ── Version targets ───────────────────────────────────────────────────────────

patch:
	bump -patch -write
	$(MAKE) build

minor:
	bump -minor -write
	$(MAKE) build

major:
	bump -major -write
	$(MAKE) build

# ── Clean ─────────────────────────────────────────────────────────────────────

clean:
	cargo clean
	@echo "🧹 Cleaned build artifacts"
