//! Sealed memory text opens with its nonce and key and nothing else.

use super::seal;
use crate::stores::sqlite::Seal;

#[test]
fn sealed_text_opens_with_its_nonce_and_not_with_another_key() {
    let Some((nonce, sealed)) = seal().seal("my student id is 1234") else {
        unreachable!("the cipher seals")
    };
    assert_ne!(sealed, b"my student id is 1234".to_vec());
    assert_eq!(
        seal().open(&nonce, &sealed).as_deref(),
        Some("my student id is 1234")
    );
    assert_eq!(Seal::new(&[8; 32]).open(&nonce, &sealed), None);
    assert_eq!(seal().open(&nonce[1..], &sealed), None);
}
