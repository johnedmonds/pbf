#![recursion_limit = "512"]

mod arrays;
mod crypto;
mod once;
mod pbf;
mod secret;

use crate::once::OnceCellContent;
use crate::pbf::PbfStats;
use crate::secret::Secret;
use arrays::make_typed_array;
use crypto::{decrypt, encrypt_secret_value, subtle, AES_CBC_PARAMS, IV_BYTES, KEY, KEY_BYTES};

use futures::FutureExt;
use std::future::Future;
use std::pin::Pin;
use wasm_bindgen::prelude::*;
use web_sys;
use web_sys::AesCbcParams;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewtil::future::LinkFuture;

type GuessSpace = char;

struct GuessState {
    // Map from character to position.
    secret: Secret<GuessSpace>,
    secret_length: usize,
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

// Reads the query portion of the url, decodes as base-64, decrypts, and returns the decrypted string.
// If anything fails (e.g. the user made an invalid url) returns an error.
// If there was no query, returns None.
fn get_secret_value() -> Option<Pin<Box<dyn Future<Output = Result<String, ()>>>>> {
    let search_text_result = web_sys::window()
        .expect("Need window feature enabled")
        .location()
        .search();
    if Ok("".to_string()) == search_text_result {
        None
    } else {
        search_text_result
            .ok()
            .map(|s| base64::decode(&s[1..])) // 1.. to skip the ? at the beginning.
            .map(|encrypted_data| {
                if let Ok(mut encrypted_data) = encrypted_data {
                    let ret: Pin<Box<dyn Future<Output = Result<String, ()>>>> =
                        Box::pin(Box::new(decrypt(&mut encrypted_data).map(
                            |decrypted_data: Result<Vec<u8>, ()>| {
                                decrypted_data.and_then(|decrypted_data: Vec<u8>| {
                                    std::str::from_utf8(&decrypted_data)
                                        .map(|s| s.to_string())
                                        .map_err(|_ignored| ())
                                })
                            },
                        )));
                    ret
                } else {
                    let ret: Pin<Box<dyn Future<Output = Result<String, ()>>>> = Box::pin(
                        Box::new(futures::future::ready::<Result<String, ()>>(Err(()))),
                    );
                    ret
                }
            })
    }
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
                    self.link.send_future(async {
                        match secret_value_future.await {
                            Ok(s) => Msg::SecretLoaded(s),
                            Err(_) => Msg::SecretLoadFailure,
                        }
                    });
                    self.mode = Mode::LoadingSecret;
                } else {
                    self.mode = Mode::CreateSecret;
                }
                true
            }
            Msg::CreateSecret => {
                let encrypted_future = encrypt_secret_value(
                    self.secret_input_ref
                        .cast::<HtmlInputElement>()
                        .unwrap()
                        .value(),
                );
                self.link.send_future(async {
                    match encrypted_future.await {
                        Ok(s) => Msg::SecretEncrypted(s),
                        Err(_) => Msg::SecretEncryptFailure,
                    }
                });
                self.mode = Mode::EncryptingSecret;
                true
            }
            Msg::SecretLoadFailure => {
                self.invalid_url = true;
                true
            }
            Msg::SecretEncryptFailure => todo!(),
            Msg::SecretLoaded(secret) => {
                self.mode = Mode::Guess(GuessState {
                    secret_length: secret.len(),
                    secret: Secret::new(secret.chars().collect()),
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
                            .cast::<HtmlInputElement>()
                            .unwrap()
                            .value(),
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
          {for guess_state.guesses.iter().map(|guess|render_guess(&guess_state.secret, guess_state.secret_length, guess))}
        </ul>
    }
}

fn render_guess(secret: &Secret<GuessSpace>, secret_length: usize, guess: &String) -> Html {
    let pbf_stats = secret.compare(&guess.chars().collect());
    let success_html = if pbf_stats.f == secret_length as i32 {
        html! {" (Correct)"}
    } else {
        html! {}
    };
    html! {<li>{guess} {" - "} {pbf_stats} {success_html} </li>}
}

#[wasm_bindgen(start)]
pub async fn run_app() {
    AES_CBC_PARAMS
        .set(OnceCellContent(AesCbcParams::new(
            "AES-CBC",
            &make_typed_array(&IV_BYTES),
        )))
        .unwrap();
    let usages_arr = js_sys::Array::new();
    usages_arr.push(&"encrypt".into());
    usages_arr.push(&"decrypt".into());
    let key = wasm_bindgen_futures::JsFuture::from(
        subtle()
            .import_key_with_str(
                "raw",
                &make_typed_array(&KEY_BYTES),
                "AES-CBC",
                false,
                &usages_arr,
            )
            .unwrap(),
    )
    .await;
    KEY.set(OnceCellContent(key.unwrap().into())).unwrap();
    App::<Model>::new().mount_to_body();
}
