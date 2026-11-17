project('zerogrep', 'rust',
  version: '0.2.0',
  meson_version: '>= 0.62.0'
)

dependency('gtk4', version: '>= 4.14')
dependency('libadwaita-1', version: '>= 1.5')

cargo_home = meson.project_build_root() / 'cargo-home'
cargo_env = environment()
cargo_env.set('CARGO_HOME', cargo_home)

cargo_target = meson.project_build_root() / 'src' / 'target'
if get_option('release')
  cargo_cmd = 'cargo build --release --manifest-path "@0@" --target-dir "@1@" && cp "@1@/release/@2@" "@OUTPUT@"'
else
  cargo_cmd = 'cargo build --manifest-path "@0@" --target-dir "@1@" && cp "@1@/debug/@2@" "@OUTPUT@"'
endif

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  env: cargo_env,
  command: [
    'sh', '-c',
    cargo_cmd.format(
      meson.project_source_root() / 'Cargo.toml',
      cargo_target,
      meson.project_name()
    )
  ]
)

install_data(
  'zerogrep.desktop',
  install_dir: get_option('datadir') / 'applications',
  rename: 'com.zerogrep.app.desktop',
)

install_data(
  'assets/zerogrep-adaptive.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps',
  rename: 'com.zerogrep.app.svg',
)
