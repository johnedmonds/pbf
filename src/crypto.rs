// Should not be on the client but it's just a game so you're on the honor system for cheating.
use crate::arrays::{array_buffer_to_vec, make_typed_array};
use crate::once::OnceCellContent;
use core::future::Future;
use futures::FutureExt;
use once_cell::sync::OnceCell;
use web_sys::AesCbcParams;
use web_sys::CryptoKey;
use web_sys::SubtleCrypto;

pub const KEY_BYTES: [u8; 16] = [
    17, 30, 228, 65, 27, 183, 113, 24, 132, 66, 33, 16, 2, 40, 129, 30,
];

// Should not be reused for different secrets but it's just a game so don't cheat.
pub const IV_BYTES: [u8; 16] = [
    211, 60, 199, 125, 214, 98, 35, 48, 13, 218, 163, 50, 33, 28, 196, 66,
];

pub static KEY: OnceCell<OnceCellContent<CryptoKey>> = OnceCell::new();
pub static AES_CBC_PARAMS: OnceCell<OnceCellContent<AesCbcParams>> = OnceCell::new();

pub fn subtle() -> SubtleCrypto {
    web_sys::window()
        .expect("Window feature must be enabled")
        .crypto()
        .expect("Crypto feature must be enabled.")
        .subtle()
}

fn encrypt(s: String) -> impl Future<Output = Result<Vec<u8>, ()>> {
    let promise = subtle().encrypt_with_object_and_buffer_source(
        &AesCbcParams::new("AES-CBC", &make_typed_array(&IV_BYTES)),
        &KEY.get().expect("Key uninitialized").0,
        &make_typed_array(s.as_bytes()),
    );
    wasm_bindgen_futures::JsFuture::from(promise.unwrap()).map(|result| {
        result
            .map(|v| array_buffer_to_vec(v.into()))
            .map_err(|_ignored| ())
    })
}

pub fn decrypt(data: &mut [u8]) -> impl Future<Output = Result<Vec<u8>, ()>> {
    let promise = subtle().decrypt_with_object_and_u8_array(
        &AES_CBC_PARAMS.get().unwrap().0,
        &KEY.get().expect("Key uninitialized").0,
        data,
    );
    wasm_bindgen_futures::JsFuture::from(promise.unwrap()).map(|result| {
        result
            .map(|v| array_buffer_to_vec(v.into()))
            .map_err(|_ignored| ())
    })
}

// Encrypts the given secret, and returns base-64 encoded encrypted data.
pub fn encrypt_secret_value(secret: String) -> impl Future<Output = Result<String, ()>> {
    encrypt(secret).map(|v| v.map(base64::encode))
}
