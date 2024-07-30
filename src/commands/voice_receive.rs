use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use dashmap::DashMap;
use hound::WavWriter;
use poise::serenity_prelude::async_trait;
use songbird::{
    model::{id::UserId, payload::ClientDisconnect},
    Event, EventContext, EventHandler as VoiceEventHandler,
};

#[derive(Clone)]
pub struct Receiver {
    inner: Arc<InnerReceiver>,
}

struct InnerReceiver {
    last_tick_was_empty: AtomicBool,
    known_ssrcs: DashMap<u32, UserId>,
}

impl Receiver {
    pub fn new() -> Self {
        // You can manage state here, such as a buffer of audio packet bytes so
        // you can later store them in intervals.
        Self {
            inner: Arc::new(InnerReceiver {
                last_tick_was_empty: AtomicBool::default(),
                known_ssrcs: DashMap::new(),
            }),
        }
    }
}

#[async_trait]
impl VoiceEventHandler for Receiver {
    #[allow(unused_variables)]
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        match ctx {
            EventContext::VoiceTick(tick) => {
                let speaking = tick.speaking.len();
                let total_participants = speaking + tick.silent.len();
                let last_tick_was_empty = self.inner.last_tick_was_empty.load(Ordering::SeqCst);

                if speaking == 0 && !last_tick_was_empty {
                    println!("No speakers");

                    self.inner.last_tick_was_empty.store(true, Ordering::SeqCst);
                } else if speaking != 0 {
                    self.inner
                        .last_tick_was_empty
                        .store(false, Ordering::SeqCst);

                    println!("Voice tick ({speaking}/{total_participants} live):");

                    for (ssrc, data) in &tick.speaking {
                        let user_id_str = if let Some(id) = self.inner.known_ssrcs.get(ssrc) {
                            format!("{:?}", *id)
                        } else {
                            "?".into()
                        };

                        if let Some(decoded_voice) = data.decoded_voice.clone() {
                            let mut writer = WavWriter::append("output.wav").unwrap();

                            for data in decoded_voice {
                                writer.write_sample(data).unwrap();
                            }

                            writer.finalize().unwrap()
                        } else {
                            println!("\t{ssrc}/{user_id_str}: Decode disabled.");
                        }
                    }
                }
            }
            EventContext::ClientDisconnect(ClientDisconnect { user_id, .. }) => {
                println!("Client disconnected: user {:?}", user_id);
            }
            _ => {
                todo!()
            }
        }
        None
    }
}
