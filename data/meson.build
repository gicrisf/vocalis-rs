# GSettings schema
gschema_conf = configuration_data()
gschema_conf.set('app-id', application_id)
gschema_conf.set('gettext-package', gettext_package)
configure_file(
  input: base_id + '.gschema.xml.in',
  output: application_id + '.gschema.xml',
  configuration: gschema_conf,
  install: true,
  install_dir: schemadir,
)

# Desktop file (translation-aware)
desktop_conf = configuration_data()
desktop_conf.set('app-id', application_id)
desktop_file = i18n.merge_file(
  type: 'desktop',
  input: configure_file(
    input: base_id + '.desktop.in.in',
    output: '@BASENAME@',
    configuration: desktop_conf,
  ),
  output: application_id + '.desktop',
  po_dir: podir,
  install: true,
  install_dir: datadir / 'applications',
)

# AppStream metainfo (translation-aware)
metainfo_conf = configuration_data()
metainfo_conf.set('app-id', application_id)
metainfo_conf.set('gettext-package', gettext_package)
metainfo_file = i18n.merge_file(
  input: configure_file(
    input: base_id + '.metainfo.xml.in.in',
    output: '@BASENAME@',
    configuration: metainfo_conf,
  ),
  output: application_id + '.metainfo.xml',
  po_dir: podir,
  install: true,
  install_dir: datadir / 'metainfo',
)

# Icons
install_data(
  'icons' / (base_id + '.svg'),
  install_dir: iconsdir / 'hicolor' / 'scalable' / 'apps',
  rename: application_id + '.svg',
)
install_data(
  'icons' / (base_id + '-symbolic.svg'),
  install_dir: iconsdir / 'hicolor' / 'symbolic' / 'apps',
  rename: application_id + '-symbolic.svg',
)
