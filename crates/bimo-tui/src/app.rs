use textual::compose;
use textual::prelude::*;

struct InputApp;

impl TextualApp for InputApp {
    fn compose(&mut self) -> AppRoot {
        AppRoot::new().with_child(Container::new().with_compose(compose![
            Input::new().with_placeholder("First Name"),
            Input::new().with_placeholder("Last Name"),
        ]))
    }
}

pub async fn run() -> Result<()> {
    textual::run(InputApp).await
}
