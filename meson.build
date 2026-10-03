project(
  'vocalis',
  'rust',
  version: '1.0.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.1',
)

gnome = import('gnome')
i18n = import('i18n')

base_id = 'app.drey.Vocalis'

cargo = find_program('cargo', required: true)

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
schemadir = datadir / 'glib-2.0' / 'schemas'
iconsdir = datadir / 'icons'
localedir = prefix / get_option('localedir')
podir = meson.project_source_root() / 'po'

# Profile / application id / cargo target.
application_id = base_id
profile = 'default'
rust_target = 'release'
cargo_profile_args = ['--release']
cargo_features_args = []
version_suffix = ''

if get_option('profile') == 'development'
  profile = 'development'
  application_id += '.Devel'
  rust_target = 'debug'
  cargo_profile_args = []
  cargo_features_args = ['--features', 'development']
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: false).stdout().strip()
  if vcs_tag == ''
    vcs_tag = 'devel'
  endif
  version_suffix = '-' + vcs_tag
endif

gettext_package = application_id

cargo_target_dir = meson.project_build_root() / 'cargo-target'
cargo_home = meson.project_build_root() / 'cargo-home'
cargo_env = [
  'CARGO_HOME=' + cargo_home,
  'CARGO_TARGET_DIR=' + cargo_target_dir,
  'VOCALIS_LOCALEDIR=' + localedir,
  'VOCALIS_VERSION_SUFFIX=' + version_suffix,
]

subdir('data')
subdir('po')
subdir('src')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
