use crate::pbf::solver::Guess;
use crate::pbf::solver::GuessState;
use crate::PbfStats;
use web_sys::HtmlInputElement;
use yew::{html, Component, ComponentLink, Html, NodeRef};

type Digit = u8;

pub struct HintComponent {
    link: ComponentLink<Self>,
    guess_state: GuessState<Digit>,
    error: Option<String>,
    hint: Option<Vec<Digit>>,
    guess_digits_ref: NodeRef,
    guess_result_ref: NodeRef,
}

pub enum Msg {
    AddGuess,
    Reset,
    ComputeHint,
}

fn get_value(node_ref: &NodeRef) -> String {
    node_ref.cast::<HtmlInputElement>().unwrap().value()
}

impl HintComponent {
    fn get_current_guess(&self) -> Result<Guess<Digit>, String> {
        let digits: Result<Vec<Digit>, String> = get_value(&self.guess_digits_ref)
            .chars()
            .map(|c| {
                c.to_digit(10)
                    .ok_or(format!("{} is not a digit", c))
                    .and_then(|i| {
                        if i <= 9 {
                            Ok(i as Digit)
                        } else {
                            Err(format!("{} must be a digit between 0 and 9 (inclusive)", i))
                        }
                    })
            })
            .collect();
        let digits = digits?;
        let results_string = get_value(&self.guess_result_ref);
        let results_p = results_string.chars().filter(|c| *c == 'p').count();
        let results_f = results_string.chars().filter(|c| *c == 'f').count();
        Ok(Guess {
            guess: digits,
            result: PbfStats {
                p: results_p as i32,
                f: results_f as i32,
            },
        })
    }

    fn render_hint(&self) -> Html {
        if let Some(hint) = &self.hint {
            html! {<p>{hint.iter().map(|digit|format!("{}",digit)).collect::<Vec<String>>().join("")}</p>}
        } else {
            html! {<p>{"No hint available."}</p>}
        }
    }
}

fn default_guess_state() -> GuessState<Digit> {
    GuessState::new((0..10).collect(), 3)
}

impl Component for HintComponent {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: yew::html::Scope<Self>) -> Self {
        Self {
            link,
            guess_state: default_guess_state(),
            error: None,
            hint: None,
            guess_digits_ref: NodeRef::default(),
            guess_result_ref: NodeRef::default(),
        }
    }
    fn update(&mut self, msg: Self::Message) -> bool {
        match msg {
            Msg::AddGuess => {
                match self.get_current_guess() {
                    Ok(guess) => {
                        self.guess_state.add_guess(guess);
                    }
                    Err(e) => {
                        self.error = Some(e);
                    }
                }
                true
            }
            Msg::Reset => {
                self.guess_state = default_guess_state();
                true
            }
            Msg::ComputeHint => {
                self.hint = self.guess_state.next_guess();
                true
            }
        }
    }
    fn change(&mut self, _: <Self as yew::Component>::Properties) -> bool {
        false
    }
    fn view(&self) -> yew::virtual_dom::VNode {
        html! {
            <div>
                <h1>{"Hints"}</h1>
                <p>{"Enter your current guesses and their results to get a hint on what to guess next."}</p>
                <h2>{"Guesses"}</h2>
                {render_guesses(&self.guess_state)}
                <label for="guess_digits">{"Guess"}</label><input type="text" ref={self.guess_digits_ref.clone()} id="guess_digits"/>
                <label for="guess_results">{"Outcome"}</label><input type="text" ref={self.guess_result_ref.clone()} id="guess_results"/>
                <input type="submit" value="Add guess" onclick={self.link.callback(|_|Msg::AddGuess)}/>
                <input type="submit" value="Clear guesses" onclick={self.link.callback(|_|Msg::Reset)}/>
                <input type="submit" value="Compute hint" onclick={self.link.callback(|_|Msg::ComputeHint)}/>
                <span class="hint">{self.render_hint()}</span>
                <span class="error">{self.error.as_ref().unwrap_or(&"".to_string())}</span>
            </div>
        }
    }
}

fn render_guesses(guess_state: &GuessState<Digit>) -> Html {
    html! {
        <ul>
          {for guess_state.guesses().iter().map(|guess|render_guess(&guess.guess, &guess.result))}
        </ul>
    }
}

fn render_guess(guess: &Vec<Digit>, results: &PbfStats) -> Html {
    html! {<li>{guess.iter().map(|c|format!("{}",c)).collect::<Vec<String>>().join("")} {" - "} {results}</li>}
}
