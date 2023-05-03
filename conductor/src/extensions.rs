use crate::coredb_crd::CoreDBExtensions;
use log::debug;

pub fn extension_plan(
    have_changed: &[CoreDBExtensions],
    actual: &[CoreDBExtensions],
) -> (Vec<CoreDBExtensions>, Vec<CoreDBExtensions>) {
    let mut changed = Vec::new();
    let mut to_install = Vec::new();

    // have_changed is unlikely to ever be >10s of extensions
    for extension_desired in have_changed {
        // check if the extension name exists in the actual list
        let mut found = false;
        // actual unlikely to be > 100s of extensions
        for extension_actual in actual {
            if extension_desired.name == extension_actual.name {
                found = true;
                // extension exists, therefore has been installed
                // determine if the `enabled` toggle has changed
                'loc: for loc_desired in extension_desired.locations.clone() {
                    for loc_actual in extension_actual.locations.clone() {
                        if loc_desired.database == loc_actual.database {
                            // TODO: when we want to support version changes, this is where we would do it
                            if loc_desired.enabled != loc_actual.enabled {
                                debug!(
                                    "desired: {:?}, actual: {:?}",
                                    extension_desired, extension_actual
                                );
                                changed.push(extension_desired.clone());
                                break 'loc;
                            }
                        }
                    }
                }
            }
        }
        // if it doesn't exist, it needs to be installed
        if !found {
            to_install.push(extension_desired.clone());
        }
    }
    debug!(
        "extension to create/drop: {:?}, extensions to install: {:?}",
        changed, to_install
    );
    (changed, to_install)
}
