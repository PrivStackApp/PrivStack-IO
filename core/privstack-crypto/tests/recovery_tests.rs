use privstack_crypto::*;

#[test]
fn round_trip_recovery_blob() {
    let master_key = generate_random_key();
    let (mnemonic, blob) = create_recovery_blob(&master_key).unwrap();

    let recovered = open_recovery_blob(&blob, &mnemonic).unwrap();
    assert_eq!(master_key.as_bytes(), recovered.as_bytes());
}

#[test]
fn wrong_mnemonic_fails() {
    let master_key = generate_random_key();
    let (_mnemonic, blob) = create_recovery_blob(&master_key).unwrap();

    let wrong = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert!(open_recovery_blob(&blob, wrong).is_err());
}

#[test]
fn round_trip_recovery_blob_with_mnemonic() {
    let master_key = generate_random_key();
    let mnemonic = privstack_crypto::envelope::generate_recovery_mnemonic().unwrap();

    let blob = create_recovery_blob_with_mnemonic(&master_key, &mnemonic).unwrap();
    let recovered = open_recovery_blob(&blob, &mnemonic).unwrap();
    assert_eq!(master_key.as_bytes(), recovered.as_bytes());
}

#[test]
fn same_mnemonic_decrypts_both_vault_blob_and_cloud_key() {
    let master_key = generate_random_key();
    let mnemonic = privstack_crypto::envelope::generate_recovery_mnemonic().unwrap();

    // Vault blob encrypted with shared mnemonic
    let vault_blob = create_recovery_blob_with_mnemonic(&master_key, &mnemonic).unwrap();

    // Cloud keypair encrypted with same mnemonic (simulated as another key)
    let cloud_secret = generate_random_key();
    let cloud_blob = create_recovery_blob_with_mnemonic(&cloud_secret, &mnemonic).unwrap();

    // Both should decrypt with the same mnemonic
    let recovered_vault = open_recovery_blob(&vault_blob, &mnemonic).unwrap();
    let recovered_cloud = open_recovery_blob(&cloud_blob, &mnemonic).unwrap();

    assert_eq!(master_key.as_bytes(), recovered_vault.as_bytes());
    assert_eq!(cloud_secret.as_bytes(), recovered_cloud.as_bytes());
}

#[test]
fn reencrypt_preserves_mnemonic_validity() {
    let old_key = generate_random_key();
    let (mnemonic, blob) = create_recovery_blob(&old_key).unwrap();

    let new_key = generate_random_key();
    let new_blob = reencrypt_recovery_blob(&blob, &mnemonic, &new_key).unwrap();

    let recovered = open_recovery_blob(&new_blob, &mnemonic).unwrap();
    assert_eq!(new_key.as_bytes(), recovered.as_bytes());
}
