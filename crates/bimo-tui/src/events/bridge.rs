use bimo_core::AgentEvent;
use cursive::CbSink;
use cursive::Cursive;
use tokio::sync::broadcast;

pub struct EventBridge {
    cb_sink: CbSink,
    rx: broadcast::Receiver<AgentEvent>,
}

impl EventBridge {
    pub fn new(cb_sink: CbSink, rx: broadcast::Receiver<AgentEvent>) -> Self {
        Self { cb_sink, rx }
    }

    pub fn spawn(self) {
        tokio::spawn(async move {
            let mut rx = self.rx;
            while let Ok(event) = rx.recv().await {
                let cb_sink = self.cb_sink.clone();
                cb_sink
                    .send(Box::new(move |siv: &mut Cursive| {
                        crate::events::handler::handle_agent_event(siv, event);
                    }))
                    .ok();
            }
        });
    }
}

pub fn create_event_bridge(siv: &mut Cursive, rx: broadcast::Receiver<AgentEvent>) -> EventBridge {
    let cb_sink = siv.cb_sink().clone();
    EventBridge::new(cb_sink, rx)
}
