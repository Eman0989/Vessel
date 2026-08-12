use std::collections::BTreeMap;
use std::path::PathBuf;

use vessel_policy::{
    CapabilityPolicy, DirectoryAccess, DirectoryCapability, NetworkAccess, PolicyError,
};

#[test]
fn default_policy_denies_all_capabilities() {
    let policy = CapabilityPolicy::default();

    assert!(policy.environment.is_empty());
    assert!(policy.directories.is_empty());
    assert_eq!(policy.network, NetworkAccess::DenyAll);

    policy.validate().unwrap();
}

#[test]
fn explicit_capabilities_are_valid() {
    let mut environment = BTreeMap::new();

    environment.insert("VESSEL_MODE".to_string(), "production".to_string());

    let policy = CapabilityPolicy {
        environment,
        directories: vec![
            DirectoryCapability {
                host_path: PathBuf::from("/srv/vessel/data"),
                guest_path: "/data".to_string(),
                access: DirectoryAccess::ReadOnly,
            },
            DirectoryCapability {
                host_path: PathBuf::from("/srv/vessel/output"),
                guest_path: "/output".to_string(),
                access: DirectoryAccess::ReadWrite,
            },
        ],
        network: NetworkAccess::DenyAll,
    };

    policy.validate().unwrap();
}

#[test]
fn duplicate_guest_paths_are_rejected() {
    let policy = CapabilityPolicy {
        environment: BTreeMap::new(),
        directories: vec![
            DirectoryCapability {
                host_path: PathBuf::from("/srv/data-a"),
                guest_path: "/data".to_string(),
                access: DirectoryAccess::ReadOnly,
            },
            DirectoryCapability {
                host_path: PathBuf::from("/srv/data-b"),
                guest_path: "/data".to_string(),
                access: DirectoryAccess::ReadWrite,
            },
        ],
        network: NetworkAccess::DenyAll,
    };

    assert_eq!(
        policy.validate(),
        Err(PolicyError::DuplicateGuestPath {
            path: "/data".to_string(),
        })
    );
}

#[test]
fn guest_parent_traversal_is_rejected() {
    let policy = CapabilityPolicy {
        environment: BTreeMap::new(),
        directories: vec![DirectoryCapability {
            host_path: PathBuf::from("/srv/data"),
            guest_path: "/data/../secrets".to_string(),
            access: DirectoryAccess::ReadOnly,
        }],
        network: NetworkAccess::DenyAll,
    };

    assert_eq!(
        policy.validate(),
        Err(PolicyError::ParentTraversal {
            path: "/data/../secrets".to_string(),
        })
    );
}

#[test]
fn invalid_environment_name_is_rejected() {
    let mut environment = BTreeMap::new();

    environment.insert("BAD=NAME".to_string(), "secret".to_string());

    let policy = CapabilityPolicy {
        environment,
        directories: Vec::new(),
        network: NetworkAccess::DenyAll,
    };

    assert_eq!(
        policy.validate(),
        Err(PolicyError::InvalidEnvironmentName {
            name: "BAD=NAME".to_string(),
        })
    );
}

#[test]
fn capability_policy_round_trips_through_json() {
    let mut environment = BTreeMap::new();

    environment.insert("MODE".to_string(), "sandbox".to_string());

    let policy = CapabilityPolicy {
        environment,
        directories: vec![DirectoryCapability {
            host_path: PathBuf::from("/srv/data"),
            guest_path: "/data".to_string(),
            access: DirectoryAccess::ReadOnly,
        }],
        network: NetworkAccess::DenyAll,
    };

    let json = serde_json::to_string(&policy).unwrap();

    let decoded: CapabilityPolicy = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, policy);
}
