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

# Run all tests across the workspace
test:
	cargo test --workspace
	@echo "✅ All tests passed"

# Run figtree tests only
test-figtree:
	cargo test --package figtree
	@echo "✅ figtree tests passed"

# Run figtree-derive tests only
test-derive:
	cargo test --package figtree-derive
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
