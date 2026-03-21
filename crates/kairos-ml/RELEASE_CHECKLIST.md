# Kairos ML Release Checklist

This checklist ensures a complete and stable release of the kairos-ml crate.

## Pre-Release Checklist

### Code Quality
- [ ] All code passes `cargo clippy -- -D warnings`
- [ ] All code is formatted with `cargo fmt`
- [ ] No `TODO`, `FIXME`, or `XXX` comments left in code
- [ ] All public APIs have documentation comments
- [ ] Unsafe code blocks are documented and justified

### Testing
- [ ] All unit tests pass: `cargo test -p kairos-ml`
- [ ] All integration tests pass: `cargo test -p kairos-ml --test '*'`
- [ ] Test coverage > 80% (run `cargo tarpaulin --out Html`)
- [ ] Benchmarks compile and run: `cargo bench -p kairos-ml`

### Documentation
- [ ] README.md is up to date with:
  - [ ] Installation instructions
  - [ ] Quick start guide
  - [ ] Usage examples
  - [ ] Troubleshooting section
- [ ] API documentation is complete: `cargo doc -p kairos-ml --no-deps`
- [ ] Examples compile and run correctly
- [ ] CHANGELOG.md is updated with all changes since last release

### Build & Dependencies
- [ ] Crate builds successfully: `cargo build -p kairos-ml --release`
- [ ] All feature flags work: `--features tch,ort`
- [ ] No dependency conflicts or version mismatches
- [ ] libtorch detection works on supported platforms

### Performance
- [ ] Inference latency < 10ms (CPU)
- [ ] Feature extraction < 5ms per 100 studies
- [ ] Training throughput > 1000 samples/second (GPU)
- [ ] Memory usage within limits (< 8GB during training)

### Integration
- [ ] CLI commands work correctly:
  - [ ] `kairos ml train`
  - [ ] `kairos ml list-models`
  - [ ] `kairos ml validate-model`
- [ ] Backtest integration works with existing engine
- [ ] Strategy lifecycle (on_init, on_candle, etc.) works correctly

### Platform Support
- [ ] Linux (x86_64) - primary platform
- [ ] macOS (x86_64, ARM64) - if applicable
- [ ] Windows (x86_64) - if applicable
- [ ] Build scripts handle missing libtorch gracefully

## Release Process

### 1. Version Bump
```bash
# Update version in Cargo.toml
# Follow semver: MAJOR.MINOR.PATCH
# MAJOR: Breaking changes
# MINOR: New features (backwards compatible)
# PATCH: Bug fixes

# Update kairos-ml/Cargo.toml
[package]
version = "0.X.0"

# Update any version-dependent code
```

### 2. Update Changelog
```bash
# Add entry to CHANGELOG.md
## [0.X.0] - YYYY-MM-DD

### Added
- New feature descriptions

### Changed
- Changed behavior descriptions

### Fixed
- Bug fix descriptions

### Deprecated
- Deprecation notices

### Removed
- Removed features
```

### 3. Create Release Commit
```bash
git add -A
git commit -m "Release kairos-ml v0.X.0"
git tag -a v0.X.0 -m "Release k0.X.0"
git push origin main --tags
```

### 4. Publish to Crates.io
```bash
# Login to crates.io
cargo login

# Publish from kairos-ml directory
cd crates/kairos-ml
cargo publish --dry-run  # Verify first
cargo publish
```

### 5. Create GitHub Release
```bash
# Use GitHub CLI
gh release create v0.X.0 \
  --title "Kairos ML v0.X.0" \
  --notes "Release notes here" \
  --draft

# Or push to trigger CI/CD release workflow
```

## Post-Release Checklist

- [ ] Verify crate is available on crates.io
- [ ] Verify documentation builds on docs.rs
- [ ] Run smoke test on clean checkout
- [ ] Update project-wide dependencies if needed
- [ ] Announce release (if applicable)

## Rollback Procedure

If a critical issue is found post-release:

1. **Immediate**: Push yank to crates.io
```bash
cargo yank --version 0.X.0 -p kairos-ml
```

2. **Fix**: Apply fixes in patch release
```bash
# Bump to 0.X.1
# Apply fixes
# Test thoroughly
# Release 0.X.1
```

3. **Communicate**: Notify users of the issue and fix

## Version Compatibility Matrix

| Kairos ML | Kairos Core | Rust | libtorch | Notes |
|-----------|-------------|------|---------|-------|
| 0.1.0 | 0.1.x | 1.75+ | 2.3+ | Initial release |
| 0.2.0 | (TBD) | (TBD) | (TBD) | (TBD) |

## Known Issues & Limitations

- [ ] List any known issues
- [ ] Note any unsupported features
- [ ] Document workaround if available

## Support Policy

- **Minimum Rust Version**: 1.75 (MSRV)
- **Supported Platforms**: Linux (primary), macOS, Windows
- **libtorch**: Required for tch feature
- **GPU Training**: Optional, requires CUDA

## Contact & Support

- **Issues**: https://github.com/jbutlerdev/kairos/issues
- **Discussions**: https://github.com/jbutlerdev/kairos/discussions
- **Documentation**: https://docs.kairos.dev/ml
