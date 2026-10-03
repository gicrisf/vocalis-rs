# The binary is built by Cargo (which also compiles the GResource via build.rs);
# Meson only drives the build and installs the result.
custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  env: cargo_env,
  command: [
    cargo,
    'build',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--locked',
  ] + cargo_profile_args + cargo_features_args + [
    '&&',
    'cp',
    cargo_target_dir / rust_target / meson.project_name(),
    '@OUTPUT@',
  ],
)

run_target(
  'cargo-clippy',
  env: cargo_env,
  command: [
    cargo,
    'clippy',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--all-targets',
  ],
)

run_target(
  'cargo-test',
  env: cargo_env,
  command: [
    cargo,
    'test',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
  ],
)
