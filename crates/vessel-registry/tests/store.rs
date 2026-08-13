use vessel_core::ArtifactRef;
use vessel_registry::{ArtifactStore, RegistryError};

#[test]
fn artifact_digest_is_deterministic_sha256() {
    let artifact = ArtifactStore::artifact_ref(b"abc");

    assert_eq!(
        artifact.digest,
        concat!(
            "sha256:",
            "ba7816bf8f01cfea414140de5dae2223",
            "b00361a396177a9cb410ff61f20015ad",
        ),
    );
}

#[test]
fn artifact_can_be_stored_and_retrieved() {
    let mut store = ArtifactStore::new();

    let bytes = b"vessel-wasm-artifact";

    let stored = store.put(bytes);

    assert_eq!(stored.size_bytes, bytes.len(),);

    assert!(store.contains(&stored.artifact));

    let retrieved = store.get(&stored.artifact).unwrap();

    assert_eq!(retrieved, bytes,);
}

#[test]
fn identical_artifacts_are_deduplicated() {
    let mut store = ArtifactStore::new();

    let first = store.put(b"same-artifact");

    let second = store.put(b"same-artifact");

    assert_eq!(first.artifact, second.artifact,);

    assert_eq!(store.len(), 1);
}

#[test]
fn different_artifacts_have_different_digests() {
    let mut store = ArtifactStore::new();

    let first = store.put(b"artifact-v1");

    let second = store.put(b"artifact-v2");

    assert_ne!(first.artifact, second.artifact,);

    assert_eq!(store.len(), 2);
}

#[test]
fn missing_artifact_returns_typed_error() {
    let store = ArtifactStore::new();

    let missing = ArtifactRef {
        digest: "sha256:missing".to_string(),
    };

    let error = store.get(&missing).unwrap_err();

    assert_eq!(
        error,
        RegistryError::ArtifactNotFound {
            digest: "sha256:missing".to_string(),
        },
    );
}
