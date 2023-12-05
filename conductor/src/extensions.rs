use controller::apis::coredb_types::CoreDB;
use controller::extensions::types::{ExtensionInstallLocation, ExtensionStatus};

fn desired_location_present_in_status(
    actual_extensions: &[ExtensionStatus],
    desired_ext_name: &str,
    desired_location: &ExtensionInstallLocation,
) -> bool {
    for actual_extension in actual_extensions {
        if actual_extension.name == desired_ext_name {
            for actual_location in &actual_extension.locations {
                if actual_location.database == desired_location.database {
                    return true;
                }
            }
            return false;
        }
    }
    false
}

pub fn extensions_still_processing(coredb: &CoreDB) -> bool {
    let actual_extensions = match &coredb.status {
        None => {
            return true;
        }
        Some(status) => {
            if status.extensionsUpdating {
                return true;
            }
            match &status.extensions {
                None => {
                    return true;
                }
                Some(extensions) => extensions,
            }
        }
    };
    let desired_extensions = &coredb.spec.extensions;
    // Return false if every desired location is present in status
    // Return true if any desired location is not located in status
    for desired_extension in desired_extensions {
        for desired_location in &desired_extension.locations {
            if !desired_location_present_in_status(
                actual_extensions,
                &desired_extension.name,
                desired_location,
            ) {
                return true;
            }
        }
    }
    false
}
