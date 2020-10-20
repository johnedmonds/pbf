#![recursion_limit = "512"]

mod crypto;
mod arrays;
mod once;

use crate::once::OnceCellContent;
use crypto::{subtle, AES_CBC_PARAMS, KEY, KEY_BYTES, IV_BYTES, encrypt_secret_value, decrypt};
use arrays::make_typed_array;

use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use wasm_bindgen::prelude::*;
use web_sys;
use web_sys::HtmlInputElement;
use web_sys::{AesCbcParams};
use yew::prelude::*;

struct GuessState {
    // Map from character to position.
    secret: Secret,
    guesses: Vec<String>,
}

enum Mode {
    Uninitialized,
    CreateSecret,
    LoadingSecret {
        success_closure: Closure<dyn FnMut(JsValue)>,
        failure_closure: Closure<dyn FnMut(JsValue)>,
    },
    EncryptingSecret {
        success_closure: Closure<dyn FnMut(JsValue)>,
        failure_closure: Closure<dyn FnMut(JsValue)>,
    },

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

struct PbfStats {
    // Number of guess characters that exist in secret but not in the right position.
    p: i32,

    // Number of guess characters that exist in secret in the same position.
    f: i32,
}

impl PbfStats {
    fn create(secret: &Secret, guess: &str) -> Self {
        let mut p = 0;
        let mut f = 0;
        for (i, c) in guess.char_indices() {
            let i = i as i32;
            let secret_char = secret.get(&c);
            if let Some(secret_char_indicies) = secret_char {
                if secret_char_indicies.contains(&i) {
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

fn secret_to_map(secret: String) -> Secret {
    let mut map: Secret = HashMap::new();
    for (i, c) in secret.char_indices() {
        map.entry(c).or_insert_with(HashSet::new).insert(i as i32);
    }
    map
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
                    let link1 = self.link.clone();
                    let link2 = self.link.clone();
                    let success_closure: Closure<dyn FnMut(JsValue)> =
                        Closure::wrap(Box::new(move |s: JsValue| {
                            link1.send_message(Msg::SecretLoaded(
                                s.as_string().expect("We passed in a string"),
                            ))
                        }));
                    let failure_closure: Closure<dyn FnMut(JsValue)> =
                        Closure::wrap(Box::new(move |_| {
                            link2.send_message(Msg::SecretLoadFailure)
                        }));
                    wasm_bindgen_futures::future_to_promise(secret_value_future.map(|result| {
                        result
                            .map(|s| JsValue::from_str(&s))
                            .map_err(|_| JsValue::NULL)
                    }))
                    .then(&success_closure)
                    .catch(&failure_closure);
                    self.mode = Mode::LoadingSecret {
                        success_closure,
                        failure_closure,
                    };
                } else {
                    self.mode = Mode::CreateSecret;
                }
                true
            }
            Msg::CreateSecret => {
                let link1 = self.link.clone();
                let link2 = self.link.clone();
                let success_closure: Closure<dyn FnMut(JsValue)> =
                    Closure::wrap(Box::new(move |s: JsValue| {
                        link1.send_message(Msg::SecretEncrypted(
                            s.as_string().expect("We passed in a string"),
                        ))
                    }));
                let failure_closure: Closure<dyn FnMut(JsValue)> =
                    Closure::wrap(Box::new(move |_| {
                        link2.send_message(Msg::SecretEncryptFailure)
                    }));

                wasm_bindgen_futures::future_to_promise(
                    encrypt_secret_value(
                        self.secret_input_ref
                            .cast::<HtmlInputElement>()
                            .unwrap()
                            .value(),
                    )
                    .map(|result| {
                        result
                            .map(|s| JsValue::from_str(&s))
                            .map_err(|_| JsValue::NULL)
                    }),
                )
                .then(&success_closure)
                .catch(&failure_closure);
                self.mode = Mode::EncryptingSecret {
                    success_closure,
                    failure_closure,
                };
                true
            }
            Msg::SecretLoadFailure => {
                self.invalid_url = true;
                true
            }
            Msg::SecretEncryptFailure => todo!(),
            Msg::SecretLoaded(secret) => {
                self.mode = Mode::Guess(GuessState {
                    secret: secret_to_map(secret),
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
                        Mode::LoadingSecret{success_closure, failure_closure} => html!{},
                        Mode::CreatedSecret(encoded_secret) => html!{<a href={format!("/?{}",encoded_secret)}>{"Click here"}</a>},
                        Mode::CreateSecret => html!{},
                        Mode::EncryptingSecret{success_closure, failure_closure} => html!{},
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

// Map of character -> set<Positions it appears in>.
type Secret = HashMap<char, HashSet<i32>>;

fn render_guess(secret: &Secret, guess: &String) -> Html {
    html! {<li>{guess} {":"} {PbfStats::create(secret, guess)}</li>}
}

#[wasm_bindgen(start)]
pub fn run_app() {
    AES_CBC_PARAMS
        .set(OnceCellContent(AesCbcParams::new(
            "AES-CBC",
            &make_typed_array(&IV_BYTES),
        )))
        .unwrap();
    let usages_arr = js_sys::Array::new();
    usages_arr.push(&"encrypt".into());
    usages_arr.push(&"decrypt".into());
    let b: Box<dyn FnMut(JsValue)> = Box::new(|ck: JsValue| {
        KEY.set(OnceCellContent(ck.into())).unwrap();
        App::<Model>::new().mount_to_body();
    });
    let c = Closure::wrap(b);
    subtle()
        .import_key_with_str(
            "raw",
            &make_typed_array(&KEY_BYTES),
            "AES-CBC",
            false,
            &usages_arr,
        )
        .unwrap()
        .then(&c);

    // Leak the closure because it needs to survive the lifetime of the program.
    c.forget();
}
