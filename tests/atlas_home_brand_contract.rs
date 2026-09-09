use sha2::{Digest, Sha256};

#[test]
fn home_uses_the_official_bitmap_and_has_no_invented_mark_or_text_hero() {
    let brand = include_str!("../src/app/widgets/atlas_brand.rs");
    let home = include_str!("../src/app/screens/atlas_home.rs");
    assert!(brand.contains("const WIDTH: usize = 29;"));
    assert!(brand.contains("const ROWS: [u32; 32]"));
    assert!(!brand.contains(&["AtlasEink", "Mark"].concat()));
    assert!(!home.contains("Capture that"));
    assert!(!home.contains("thought."));
    assert_eq!(
        format!("{:x}", Sha256::digest(brand.as_bytes())),
        "3fa589b589f1d084d68689b9c6eb271d107972b7dd38d192e53c8c6ed3104184"
    );
}
