#![recursion_limit = "512"]

use futures::FutureExt;
use js_sys::ArrayBuffer;
use js_sys::Uint8Array;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use wasm_bindgen::prelude::*;
use web_sys;
use web_sys::SubtleCrypto;
use web_sys::{AesCbcParams, CryptoKey};
use yew::prelude::*;

// Should not be on the client but it's just a game so you're on the honor system for cheating.
const KEY_BYTES: [u8; 16] = [
    17, 30, 228, 65, 27, 183, 113, 24, 132, 66, 33, 16, 2, 40, 129, 30,
];

// Should not be reused for different secrets but it's just a game so don't cheat.
const IV_BYTES: [u8; 16] = [
    211, 60, 199, 125, 214, 98, 35, 48, 13, 218, 163, 50, 33, 28, 196, 66,
];

fn make_typed_array(arr: &[u8]) -> Uint8Array {
    let out_arr = Uint8Array::new_with_length(arr.len() as u32);
    for i in 0..arr.len() {
        out_arr.set_index(i as u32, arr[i])
    }
    out_arr
}

fn subtle() -> SubtleCrypto {
    web_sys::window()
        .expect("Window feature must be enabled")
        .crypto()
        .expect("Crypto feature must be enabled.")
        .subtle()
}

fn make_aes_cbc_params() -> AesCbcParams {
    AesCbcParams::new("AES-CBC", &make_typed_array(&IV_BYTES))
}

struct GuessState {
    // Map from character to position.
    secret: HashMap<char, i32>,
    guesses: Vec<String>,
}

enum Mode {
    Uninitialized,
    CreateSecret,
    LoadingSecret,
    EncryptingSecret,

    // Value is the base64-encoded, encrypted, secret.
    CreatedSecret(String),
    Guess(GuessState),
}

struct Model {
    link: ComponentLink<Self>,
    key: Option<CryptoKey>,
    mode: Mode,
    invalid_url: bool,
    secret_input_ref: NodeRef,
    next_guess_input_ref: NodeRef,
}

enum Msg {
    CreateSecret,
    Guess,
    StartLoadingSecret,
    SecretLoadFailure,
    SecretEncryptFailure,
    SecretLoaded(String),
    SecretEncrypted(String),
}

struct PbfStats {
    // Number of guess characters that exist in secret but not in the right position.
    p: i32,

    // Number of guess characters that exist in secret in the same position.
    f: i32,
}

impl PbfStats {
    fn create(secret: &HashMap<char, i32>, guess: &str) -> Self {
        let mut p = 0;
        let mut f = 0;
        for (i, c) in guess.char_indices() {
            let i = i as i32;
            let secret_char = secret.get(&c);
            if let Some(secret_char_index) = secret_char {
                if *secret_char_index == i {
                    f = f + 1;
                } else {
                    p = p + 1;
                }
            }
        }
        PbfStats { p, f }
    }
}

impl Display for PbfStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.p == 0 && self.f == 0 {
            write!(f, "b")
        } else {
            write!(
                f,
                "{}{}",
                vec!["f"; self.f as usize].join(""),
                vec!["p"; self.p as usize].join("")
            )
        }
    }
}

fn array_buffer_to_vec(arr: ArrayBuffer) -> Vec<u8> {
    let arr = Uint8Array::new_with_byte_offset(&arr, 0);
    let mut out: Vec<u8> = Vec::with_capacity(arr.length() as usize);
    for i in 0..arr.length() {
        out.push(arr.get_index(i));
    }
    out
}

fn encrypt(s: String) -> impl Future<Output = Result<Vec<u8>, ()>> {
    let promise = subtle().encrypt_with_str_and_buffer_source(
        "AES-CBC",
        &KEY.get().expect("Key uninitialized").key,
        &make_typed_array(s.as_bytes()),
    );
    wasm_bindgen_futures::JsFuture::from(promise.unwrap()).map(|result| {
        result
            .map(|v| array_buffer_to_vec(v.into()))
            .map_err(|_ignored| ())
    })
}

fn decrypt(data: &mut [u8]) -> impl Future<Output = Result<Vec<u8>, ()>> {
    let promise = subtle().decrypt_with_str_and_u8_array(
        "AES-CBC",
        &KEY.get().expect("Key uninitialized").key,
        data,
    );
    wasm_bindgen_futures::JsFuture::from(promise.unwrap()).map(|result| {
        result
            .map(|v| array_buffer_to_vec(v.into()))
            .map_err(|_ignored| ())
    })
}

// Encrypts the given secret, and returns base-64 encoded encrypted data.
fn encrypt_secret_value(secret: String) -> impl Future<Output = Result<String, ()>> {
    encrypt(secret).map(|v| v.map(base64::encode))
}

// Reads the query portion of the url, decodes as base-64, decrypts, and returns the decrypted string.
// If anything fails (e.g. the user made an invalid url) returns an error.
// If there was no query, returns None.
fn get_secret_value() -> Option<Pin<Box<dyn Future<Output = Result<String, ()>>>>> {
    web_sys::window()
        .expect("Need window feature enabled")
        .location()
        .search()
        .ok()
        .map(base64::decode)
        .map(|encrypted_data| {
            if let Ok(mut encrypted_data) = encrypted_data {
                let ret: Pin<Box<dyn Future<Output = Result<String, ()>>>> = Box::pin(Box::new(
                    decrypt(&mut encrypted_data).map(|decrypted_data: Result<Vec<u8>, ()>| {
                        decrypted_data.and_then(|decrypted_data: Vec<u8>| {
                            std::str::from_utf8(&decrypted_data)
                                .map(|s| s.to_string())
                                .map_err(|_ignored| ())
                        })
                    }),
                ));
                ret
            } else {
                let ret: Pin<Box<dyn Future<Output = Result<String, ()>>>> = Box::pin(Box::new(
                    futures::future::ready::<Result<String, ()>>(Err(())),
                ));
                ret
            }
        })
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        let secret_input_ref = NodeRef::default();
        let next_guess_input_ref = NodeRef::default();
        link.send_message(Msg::StartLoadingSecret);
        Self {
            link,
            invalid_url: false,
            key: None,
            mode: Mode::Uninitialized,
            secret_input_ref,
            next_guess_input_ref,
        }
    }
    fn update(&mut self, msg: <Self as yew::Component>::Message) -> bool {
        match msg {
            Msg::StartLoadingSecret => {
                let secret_value_result = get_secret_value();
                if let Some(secret_value_future) = secret_value_result {
                    let link1 = self.link.clone();
                    let link2 = self.link.clone();
                    self.mode = Mode::LoadingSecret;
                    wasm_bindgen_futures::future_to_promise(secret_value_future.map(|result| {
                        result
                            .map(|s| JsValue::from_str(&s))
                            .map_err(|_| JsValue::NULL)
                    }))
                    .then(&Closure::wrap(Box::new(move |s: JsValue| {
                        link1.send_message(Msg::SecretLoaded(
                            s.as_string().expect("We passed in a string"),
                        ))
                    })))
                    .catch(&Closure::wrap(Box::new(move |_| {
                        link2.send_message(Msg::SecretLoadFailure)
                    })));
                }
                true
            }
            Msg::CreateSecret => {
                self.mode = Mode::EncryptingSecret;
                let link1 = self.link.clone();
                let link2 = self.link.clone();
                wasm_bindgen_futures::future_to_promise(
                    encrypt_secret_value(
                        self.secret_input_ref
                            .get()
                            .unwrap()
                            .node_value()
                            .unwrap_or("".to_string()),
                    )
                    .map(|result| {
                        result
                            .map(|s| JsValue::from_str(&s))
                            .map_err(|_| JsValue::NULL)
                    }),
                )
                .then(&Closure::wrap(Box::new(move |s: JsValue| {
                    link1.send_message(Msg::SecretEncrypted(
                        s.as_string().expect("We passed in a string"),
                    ))
                })))
                .catch(&Closure::wrap(Box::new(move |_| {
                    link2.send_message(Msg::SecretEncryptFailure)
                })));
                true
            }
            Msg::SecretLoadFailure => {
                self.invalid_url = true;
                true
            }
            Msg::SecretEncryptFailure => todo!(),
            Msg::SecretLoaded(secret) => {
                self.mode = Mode::Guess(GuessState {
                    secret: secret.char_indices().map(|(i, c)| (c, i as i32)).collect(),
                    guesses: Vec::new(),
                });
                true
            }
            Msg::SecretEncrypted(secret) => {
                self.mode = Mode::CreatedSecret(secret);
                true
            }
            Msg::Guess => {
                if let Mode::Guess(ref mut guess_state) = self.mode {
                    guess_state.guesses.push(
                        self.next_guess_input_ref
                            .get()
                            .unwrap()
                            .node_value()
                            .unwrap_or("".to_string()),
                    );
                    true
                } else {
                    panic!("Guessing while not in guess state");
                }
            }
        }
    }
    fn change(&mut self, _: <Self as yew::Component>::Properties) -> bool {
        false // no properties to change.
    }
    fn view(&self) -> yew::virtual_dom::VNode {
        html! {
            <div>
                {
                    if self.invalid_url {
                        html!{<p>{"Invalid url"}</p>}
                    } else {
                        html!{}
                    }
                }
                <p>{"Create a new game"}</p>
                <label for={"secret_number_input"}>{"Secret number"}</label>
                <input type="text" ref={self.secret_input_ref.clone()} id={"secret_number_input"}/>
                <input type="submit" onclick=self.link.callback(|_|Msg::CreateSecret)/>
                {
                    match &self.mode {
                        Mode::Uninitialized => html!{},
                        Mode::LoadingSecret => html!{},
                        Mode::CreatedSecret(encoded_secret) => html!{<a href={format!("/?{}",encoded_secret)}>{"Click here"}</a>},
                        Mode::CreateSecret => html!{},
                        Mode::EncryptingSecret => html!{},
                        Mode::Guess(guess_state) => html!{
                            <div class="guesses">
                                {render_guesses(guess_state)}
                                <label for="next_guess">{"Next guess"}</label>
                                <input type="text" id="next_guess" ref={self.next_guess_input_ref.clone()}/>
                                <input type="submit" onclick=self.link.callback(|_|Msg::Guess)/>
                            </div>
                        }
                    }
                }
            </div>
        }
    }
}
fn render_guesses(guess_state: &GuessState) -> Html {
    html! {
        <ul>
          {for guess_state.guesses.iter().map(|guess|render_guess(&guess_state.secret, guess))}
        </ul>
    }
}

fn render_guess(secret: &HashMap<char, i32>, guess: &String) -> Html {
    html! {<li>{guess} {":"} {PbfStats::create(secret, guess)}</li>}
}

#[derive(Debug)]
struct KeyBox {
    key: CryptoKey,
}

unsafe impl Send for KeyBox {}
unsafe impl Sync for KeyBox {}

static KEY: OnceCell<KeyBox> = OnceCell::new();
#[wasm_bindgen(start)]
pub fn run_app() {
    let usages_arr = js_sys::Array::new();
    usages_arr.push(&"encrypt".into());
    usages_arr.push(&"decrypt".into());
    subtle()
        .import_key_with_str(
            "raw",
            &make_typed_array(&KEY_BYTES),
            "AES-CBC",
            false,
            &usages_arr,
        )
        .unwrap()
        .then(&Closure::wrap(Box::new(|ck| {
            KEY.set(KeyBox { key: ck.into() }).unwrap();
            App::<Model>::new().mount_to_body();
        })));
}
